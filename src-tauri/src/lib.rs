mod commands;
mod error;
mod state;

use specta_typescript::Typescript;
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

/// Filename of the global metadata database in the app data directory.
/// Per-project databases live elsewhere and are opened on demand.
const METADATA_DB_FILENAME: &str = "mango.db";

pub fn run() {
    let builder = Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::project::create_project,
        commands::project::get_project,
        commands::project::list_projects,
        commands::project::update_project,
        commands::project::delete_project,
    ]);

    #[cfg(debug_assertions)]
    builder
        .export(
            Typescript::default(),
            "../src/lib/bindings/commands.ts",
        )
        .expect("Failed to export tauri-specta typescript bindings");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);

            let app_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_dir)?;
            let db_path = app_dir.join(METADATA_DB_FILENAME);

            let db = tauri::async_runtime::block_on(mango_core::db::open_async(&db_path))
                .expect("Failed to initialize metadata database");

            app.manage(state::AppState { db });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
