use super::{require_mount, with_db};
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::{timeline_item, timeline_track};
use mango_core::models::timeline::*;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn list_timeline_tracks(
    state: State<'_, AppState>,
    episode_id: String,
) -> Result<Vec<TimelineTrack>, IpcError> {
    with_db(&state, move |conn| timeline_track::list_by_episode(conn, &episode_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_timeline_track(
    state: State<'_, AppState>,
    input: CreateTimelineTrackInput,
) -> Result<TimelineTrack, IpcError> {
    with_db(&state, move |conn| timeline_track::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_timeline_track(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| timeline_track::delete(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn reorder_timeline_tracks(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| timeline_track::reorder(conn, &ids)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_default_tracks(
    state: State<'_, AppState>,
    episode_id: String,
) -> Result<Vec<TimelineTrack>, IpcError> {
    with_db(&state, move |conn| timeline_track::create_defaults(conn, &episode_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_timeline_items(
    state: State<'_, AppState>,
    track_id: String,
) -> Result<Vec<TimelineItem>, IpcError> {
    with_db(&state, move |conn| timeline_item::list_by_track(conn, &track_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_timeline_item(
    state: State<'_, AppState>,
    input: CreateTimelineItemInput,
) -> Result<TimelineItem, IpcError> {
    with_db(&state, move |conn| timeline_item::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_timeline_item(
    state: State<'_, AppState>,
    id: String,
    input: UpdateTimelineItemInput,
) -> Result<TimelineItem, IpcError> {
    with_db(&state, move |conn| timeline_item::update(conn, &id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn move_timeline_item(
    state: State<'_, AppState>,
    id: String,
    input: MoveTimelineItemInput,
) -> Result<TimelineItem, IpcError> {
    with_db(&state, move |conn| timeline_item::move_item(conn, &id, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_timeline_item(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| timeline_item::delete(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn import_video_from_clips(
    state: State<'_, AppState>,
    episode_id: String,
) -> Result<Vec<TimelineItem>, IpcError> {
    let mounted = require_mount(&state)?;
    let db = mounted.db.clone();
    let workspace_root = mounted.workspace_root.clone();

    let items = db
        .call(move |conn| {
            let config = mango_core::ffmpeg::FfmpegConfig::from_env();
            Ok(mango_core::timeline::import_video_from_clips(
                conn,
                &episode_id,
                &workspace_root,
                &config,
            ))
        })
        .await
        .map_err(IpcError::from)?
        .map_err(IpcError::from)?;

    Ok(items)
}

#[tauri::command]
#[specta::specta]
pub async fn import_audio_from_shots(
    state: State<'_, AppState>,
    episode_id: String,
) -> Result<Vec<TimelineItem>, IpcError> {
    let mounted = require_mount(&state)?;
    let db = mounted.db.clone();
    let workspace_root = mounted.workspace_root.clone();

    let items = db
        .call(move |conn| {
            let config = mango_core::ffmpeg::FfmpegConfig::from_env();
            Ok(mango_core::timeline::import_audio_from_shots(
                conn,
                &episode_id,
                &workspace_root,
                &config,
            ))
        })
        .await
        .map_err(IpcError::from)?
        .map_err(IpcError::from)?;

    Ok(items)
}
