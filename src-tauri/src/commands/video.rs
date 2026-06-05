use std::path::Path;

use tauri::AppHandle;
use tauri_specta::Event;

use super::{require_mount, with_db};
use crate::error::IpcError;
use crate::events::FfmpegProgressTick;
use crate::state::AppState;
use mango_core::db::queries::video_clip as clip_queries;
use mango_core::ffmpeg::FfmpegProgress;
use mango_core::models::video_clip::{CreateVideoClipInput, UpdateVideoClipInput, VideoClip};
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_video_clips(
    state: State<'_, AppState>,
    episode_id: String,
) -> Result<Vec<VideoClip>, IpcError> {
    with_db(&state, move |conn| clip_queries::list(conn, &episode_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_video_clip(
    state: State<'_, AppState>,
    input: CreateVideoClipInput,
) -> Result<VideoClip, IpcError> {
    with_db(&state, move |conn| clip_queries::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_video_clip(
    state: State<'_, AppState>,
    id: String,
    input: UpdateVideoClipInput,
) -> Result<VideoClip, IpcError> {
    with_db(&state, move |conn| clip_queries::update(conn, &id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_video_clip(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| clip_queries::delete(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn reorder_video_clips(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| clip_queries::reorder(conn, &ids)).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_video_clips(
    app: AppHandle,
    state: State<'_, AppState>,
    episode_id: String,
    output_path: String,
) -> Result<String, IpcError> {
    let mounted = require_mount(&state)?;
    let db = mounted.db.clone();
    let workspace_root = mounted.workspace_root.clone();

    let result: mango_core::error::Result<std::path::PathBuf> = db
        .call(move |conn| {
            let config = mango_core::ffmpeg::FfmpegConfig::from_env();
            let mut cb = |p: FfmpegProgress| {
                let _ = FfmpegProgressTick {
                    progress_pct: p.progress_pct,
                    current_time_ms: p.current_time_ms,
                    total_duration_ms: p.total_duration_ms,
                    speed: p.speed,
                }
                .emit(&app);
            };
            Ok(mango_core::export::export_video_clips(
                conn,
                &config,
                &episode_id,
                &workspace_root,
                Path::new(&output_path),
                Some(&mut cb),
            ))
        })
        .await
        .map_err(IpcError::from)?;

    result
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(IpcError::from)
}
