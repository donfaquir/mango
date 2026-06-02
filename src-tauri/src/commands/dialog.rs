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

// `suggest_project_root` was removed. The CreateProjectDialog no longer asks
// the user to pick a directory — it just takes a subdirectory name that
// lands under `<workspace>/projects/`, defaulting to a slug of the project
// name. See `paths::make_relative_project_root`.
