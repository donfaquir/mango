mod commands;
mod error;
mod events;
mod init;
mod state;

use std::sync::{Arc, OnceLock};

use mango_core::account::keyring::{CachedKeyringStore, SystemKeyring};
#[cfg(any(debug_assertions, test))]
use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Builder};
use tracing_subscriber::EnvFilter;

fn make_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::account::create_api_account,
            commands::account::delete_api_account,
            commands::account::get_api_account,
            commands::account::list_api_accounts,
            commands::account::update_api_account,
            commands::account::verify_api_account_storage,
            commands::asset::assign_asset_to_shot,
            commands::asset::delete_asset,
            commands::asset::find_asset_by_path,
            commands::asset::get_asset,
            commands::asset::import_asset,
            commands::asset::list_asset_labels,
            commands::asset::list_assets,
            commands::asset::update_asset_label,
            commands::asset::update_asset_original_name,
            commands::canvas_layout::delete_canvas_layout,
            commands::canvas_layout::get_canvas_layout,
            commands::canvas_layout::upsert_canvas_layout,
            commands::character::create_character,
            commands::character::delete_character,
            commands::character::get_character,
            commands::character::list_characters,
            commands::character::update_character,
            commands::costume::create_costume,
            commands::costume::delete_costume,
            commands::costume::get_costume,
            commands::costume::list_costumes,
            commands::costume::update_costume,
            commands::dialog::pick_image_file,
            commands::dialog::pick_project_directory,
            commands::episode::create_episode,
            commands::episode::delete_episode,
            commands::episode::get_episode,
            commands::episode::list_episodes,
            commands::episode::reorder_episodes,
            commands::episode::update_episode,
            commands::episode_checkpoint::create_episode_checkpoint,
            commands::project::create_project,
            commands::project::delete_project,
            commands::project::get_project,
            commands::project::list_projects,
            commands::project::update_project,
            commands::prop::create_prop,
            commands::prop::delete_prop,
            commands::prop::get_prop,
            commands::prop::list_props,
            commands::prop::update_prop,
            commands::provider::list_models,
            commands::provider::list_providers,
            commands::scene::create_scene,
            commands::scene::delete_scene,
            commands::scene::get_scene,
            commands::scene::list_scenes,
            commands::scene::update_scene,
            commands::shot::create_shot,
            commands::shot::delete_shot,
            commands::shot::get_shot,
            commands::shot::link_shot_subject,
            commands::shot::list_shot_links,
            commands::shot::list_shots,
            commands::shot::reorder_shots,
            commands::shot::unlink_shot_subject,
            commands::shot::update_shot,
            commands::task::cancel_task,
            commands::task::get_task,
            commands::task::get_task_max_concurrency,
            commands::task::list_task_events,
            commands::task::list_tasks,
            commands::task::set_task_max_concurrency,
            commands::task::submit_task,
            commands::task::submit_tasks_batch,
            commands::workspace::get_workspace_status,
            commands::workspace::mount_workspace,
            commands::workspace::probe_workspace,
            commands::workspace::set_workspace_and_relaunch,
        ])
        .events(collect_events![
            events::TaskStatusChanged,
            events::TaskEventLogged,
            events::TaskProgressTick,
        ])
}

pub fn run() {
    // Default to INFO so migration / orphan-reset / error logs are visible
    // without setting RUST_LOG. Users can narrow with RUST_LOG=mango_core=debug etc.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    let builder = make_builder();

    #[cfg(debug_assertions)]
    builder
        .export(
            Typescript::default(),
            "../src/lib/bindings/commands.ts",
        )
        .expect("Failed to export tauri-specta typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            // Register the platform's native credential store as keyring-core's
            // default before we hand out any `SystemKeyring` handles. `false`
            // selects keyutils on Linux (the Secret Service alternative needs
            // dbus and a session bus, which not all setups have).
            keyring::use_native_store(false)
                .expect("Failed to register the native keyring store");

            let app_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_dir)?;

            // Wrap the OS-backed keyring in a session-level cache so repeated
            // fetches within one app session don't each trigger a macOS
            // Keychain authorization popup.
            let keyring: Arc<dyn mango_core::account::keyring::KeyringStore> =
                Arc::new(CachedKeyringStore::new(Box::new(SystemKeyring)));

            // Resolve the user-chosen workspace, if any. A missing or invalid
            // pointer config means we boot into onboarding mode: `mounted`
            // stays `None`, every business command short-circuits with
            // `WORKSPACE_NOT_MOUNTED`, and the frontend routes the user to
            // the onboarding flow. `mount_workspace` (called from the
            // onboarding UI) fills the OnceLock at runtime — no restart
            // needed.
            let workspace_root: Option<std::path::PathBuf> =
                match mango_core::app_config::read(&app_dir) {
                    Ok(Some(cfg)) => {
                        let p = cfg.workspace_path;
                        if p.is_dir() && p.join(init::METADATA_DB_FILENAME).is_file() {
                            Some(p)
                        } else {
                            tracing::warn!(
                                workspace = %p.display(),
                                "configured workspace path is invalid or missing mango.db; \
                                 booting into onboarding mode"
                            );
                            None
                        }
                    }
                    Ok(None) => None,
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to read app_config; booting into onboarding mode");
                        None
                    }
                };

            let mounted_cell: OnceLock<state::MountedState> = OnceLock::new();
            if let Some(ws) = workspace_root {
                let app_handle = app.handle().clone();
                let keyring_for_mount = keyring.clone();
                // Run the boot-time mount synchronously: a workspace that's
                // configured must be ready before any command can fire,
                // otherwise the frontend would briefly think it's in
                // onboarding mode.
                let mounted = tauri::async_runtime::block_on(init::init_workspace(
                    &app_handle,
                    keyring_for_mount,
                    &ws,
                ))
                .expect("failed to mount workspace");
                mounted_cell
                    .set(mounted)
                    .map_err(|_| ())
                    .expect("OnceLock was unexpectedly already set");
            }

            app.manage(state::AppState {
                app_data_dir: app_dir,
                keyring,
                mounted: mounted_cell,
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regenerate `src/lib/bindings/commands.ts` whenever Rust IPC surfaces
    /// change. Running `cargo test -p mango-tauri export_bindings` keeps the
    /// TS bindings in sync without launching the full GUI.
    #[test]
    fn export_bindings() {
        make_builder()
            .export(Typescript::default(), "../src/lib/bindings/commands.ts")
            .expect("Failed to export tauri-specta typescript bindings");
    }
}
