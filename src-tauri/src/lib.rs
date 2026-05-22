mod commands;
mod error;
mod state;

use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

/// Filename of the global metadata database in the app data directory.
/// Per-project databases live elsewhere and are opened on demand.
const METADATA_DB_FILENAME: &str = "mango.db";

fn make_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::dialog::pick_project_directory,
        commands::dialog::suggest_project_root,
        commands::project::create_project,
        commands::project::get_project,
        commands::project::list_projects,
        commands::project::update_project,
        commands::project::delete_project,
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

            app.manage(state::AppState {
                db,
                app_data_dir: app_dir,
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
