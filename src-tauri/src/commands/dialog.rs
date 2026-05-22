use tauri::Manager;
use tauri_plugin_dialog::{DialogExt, FilePath};
use tokio::sync::oneshot;

use crate::error::IpcError;

/// Open a native directory picker. Returns `None` if the user cancelled.
///
/// Implemented with the non-blocking callback variant + a oneshot channel so
/// the tokio executor thread is not parked while the user is choosing.
#[tauri::command]
#[specta::specta]
pub async fn pick_project_directory(
    app: tauri::AppHandle,
) -> Result<Option<String>, IpcError> {
    let (tx, rx) = oneshot::channel();
    app.dialog().file().pick_folder(move |result| {
        let mapped = result.map(|fp| match fp {
            FilePath::Path(p) => p.to_string_lossy().into_owned(),
            FilePath::Url(u) => u.to_string(),
        });
        let _ = tx.send(mapped);
    });
    rx.await
        .map_err(|e| IpcError::internal(format!("dialog channel closed: {e}")))
}

/// Open a native file picker filtered to common image extensions. Returns
/// `None` if the user cancelled. Mirrors `pick_project_directory` so the
/// frontend keeps a single picker pattern across the app.
#[tauri::command]
#[specta::specta]
pub async fn pick_image_file(
    app: tauri::AppHandle,
) -> Result<Option<String>, IpcError> {
    let (tx, rx) = oneshot::channel();
    app.dialog()
        .file()
        .add_filter("图片", &["png", "jpg", "jpeg", "webp"])
        .pick_file(move |result| {
            let mapped = result.map(|fp| match fp {
                FilePath::Path(p) => p.to_string_lossy().into_owned(),
                FilePath::Url(u) => u.to_string(),
            });
            let _ = tx.send(mapped);
        });
    rx.await
        .map_err(|e| IpcError::internal(format!("dialog channel closed: {e}")))
}

/// Suggest a default project root for a given (display) name. The returned
/// path is `<app_data>/projects/<slug>` where slug is name-derived for human
/// readability; the actual persisted root is whatever the user submits.
#[tauri::command]
#[specta::specta]
pub async fn suggest_project_root(
    app: tauri::AppHandle,
    project_name: String,
) -> Result<String, IpcError> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|e| IpcError::internal(e.to_string()))?;

    let slug = slug::slugify(&project_name);
    let dir_name = if slug.is_empty() { "untitled" } else { slug.as_str() };
    let dir = app_data.join("projects").join(dir_name);
    Ok(dir.to_string_lossy().into_owned())
}
