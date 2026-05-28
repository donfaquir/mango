mod commands;
mod error;
mod events;
mod state;

use std::sync::Arc;

use mango_core::account::keyring::{CachedKeyringStore, SystemKeyring};
use mango_core::provider::bailian::BailianProvider;
use mango_core::provider::bailian::materializer::BailianResultMaterializer;
use mango_core::provider::ProviderRegistry;
use mango_core::task_engine::{TaskEngineHandle, TaskEvent};
use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Builder};
use tauri_specta::Event;

/// Filename of the global metadata database in the app data directory.
/// Per-project databases live elsewhere and are opened on demand.
const METADATA_DB_FILENAME: &str = "mango.db";

fn make_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::account::create_api_account,
            commands::account::delete_api_account,
            commands::account::get_api_account,
            commands::account::list_api_accounts,
            commands::account::update_api_account,
            commands::account::verify_api_account_storage,
            commands::asset::delete_asset,
            commands::asset::find_asset_by_path,
            commands::asset::get_asset,
            commands::asset::import_asset,
            commands::asset::list_assets,
            commands::asset::register_project_asset_scope,
            commands::asset::update_asset_label,
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
            commands::dialog::suggest_project_root,
            commands::episode::create_episode,
            commands::episode::delete_episode,
            commands::episode::get_episode,
            commands::episode::list_episodes,
            commands::episode::reorder_episodes,
            commands::episode::update_episode,
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
            commands::task::list_task_events,
            commands::task::list_tasks,
            commands::task::submit_task,
        ])
        .events(collect_events![
            events::TaskStatusChanged,
            events::TaskEventLogged,
            events::TaskProgressTick,
        ])
}

pub fn run() {
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
            let db_path = app_dir.join(METADATA_DB_FILENAME);

            let db = tauri::async_runtime::block_on(mango_core::db::open_async(&db_path))
                .expect("Failed to initialize metadata database");

            // Unified startup hook (backfill legacy root_path NULL rows, etc.).
            // Hard failure here is intentional — a half-initialized DB leads to
            // user-visible crashes that are far harder to diagnose later.
            // tokio-rusqlite 0.7 has no Other variant on its Error, so we
            // surface the CoreError through the Ok(...) channel like with_db.
            let app_data_for_init = app_dir.clone();
            let init_result: mango_core::error::Result<()> = tauri::async_runtime::block_on(async {
                db.call(
                    move |conn| -> std::result::Result<
                        mango_core::error::Result<()>,
                        rusqlite::Error,
                    > {
                        Ok(mango_core::startup::initialize(conn, &app_data_for_init))
                    },
                )
                .await
                .expect("startup initialize call failed on DB thread")
            });
            init_result.expect("Failed to run startup initialize");

            // Register BailianProvider for the "bailian" provider id.
            let providers = ProviderRegistry::builder()
                .register("bailian", Arc::new(BailianProvider::new()))
                .build();
            // Wrap the OS-backed keyring in a session-level cache so repeated
            // fetches within one app session don't each trigger a macOS
            // Keychain authorization popup.
            let keyring: Arc<dyn mango_core::account::keyring::KeyringStore> =
                Arc::new(CachedKeyringStore::new(Box::new(SystemKeyring)));
            let materializer: Arc<dyn mango_core::task_engine::ResultMaterializer> =
                Arc::new(BailianResultMaterializer::new(db.clone(), keyring.clone()));
            let (engine, mut event_rx) = TaskEngineHandle::spawn(
                db.clone(),
                providers,
                keyring.clone(),
                materializer,
                4,
            );

            // Re-spawn runner coroutines for any pending tasks left over from
            // a previous session. Runs in the background so setup() returns
            // promptly even if the DB has many pending rows.
            let engine_for_recovery = engine.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = engine_for_recovery.recover_pending().await {
                    tracing::error!("failed to recover pending tasks: {e}");
                }
            });

            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                while let Some(ev) = event_rx.recv().await {
                    match ev {
                        TaskEvent::StatusChanged {
                            task_id,
                            status,
                            progress,
                            error_message,
                        } => {
                            if let Err(e) = (events::TaskStatusChanged {
                                task_id,
                                status,
                                progress,
                                error_message,
                            })
                            .emit(&app_handle)
                            {
                                tracing::warn!(error = %e, "failed to emit TaskStatusChanged");
                            }
                        }
                        TaskEvent::EventLogged { task_id, event } => {
                            if let Err(e) = (events::TaskEventLogged { task_id, event })
                                .emit(&app_handle)
                            {
                                tracing::warn!(error = %e, "failed to emit TaskEventLogged");
                            }
                        }
                        TaskEvent::ProgressTick { task_id, progress } => {
                            if let Err(e) = (events::TaskProgressTick { task_id, progress })
                                .emit(&app_handle)
                            {
                                tracing::warn!(error = %e, "failed to emit TaskProgressTick");
                            }
                        }
                    }
                }
            });

            app.manage(state::AppState {
                db,
                app_data_dir: app_dir,
                keyring,
                task_engine: Arc::new(engine),
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
