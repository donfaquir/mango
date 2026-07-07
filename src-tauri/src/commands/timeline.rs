use serde::Serialize;
use specta::Type;

use super::{require_mount, with_db};
use crate::error::IpcError;
use crate::state::AppState;
use mango_core::db::queries::{timeline_item, timeline_keyframe, timeline_track};
use mango_core::models::timeline::*;
use tauri::State;

#[derive(Debug, Clone, Serialize, Type)]
pub struct KenBurnsPresetInfo {
    pub id: String,
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct TransitionPresetInfo {
    pub id: String,
    pub label: String,
    pub xfade_name: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct StickerPresetInfo {
    pub id: String,
    pub label: String,
    pub category: String,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct ColorEffectPresetInfo {
    pub id: String,
    pub label: String,
}

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

#[tauri::command]
#[specta::specta]
pub async fn update_track_muted(
    state: State<'_, AppState>,
    id: String,
    muted: bool,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| timeline_track::update_muted(conn, &id, muted)).await
}

#[tauri::command]
#[specta::specta]
pub async fn update_track_locked(
    state: State<'_, AppState>,
    id: String,
    locked: bool,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| timeline_track::update_locked(conn, &id, locked)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_keyframes(
    state: State<'_, AppState>,
    item_id: String,
) -> Result<Vec<TimelineKeyframe>, IpcError> {
    with_db(&state, move |conn| timeline_keyframe::list_by_item(conn, &item_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn create_keyframe(
    state: State<'_, AppState>,
    input: CreateTimelineKeyframeInput,
) -> Result<TimelineKeyframe, IpcError> {
    with_db(&state, move |conn| timeline_keyframe::create(conn, input)).await
}

#[tauri::command]
#[specta::specta]
pub async fn delete_keyframe(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), IpcError> {
    with_db(&state, move |conn| timeline_keyframe::delete(conn, &id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_ken_burns_presets() -> Result<Vec<KenBurnsPresetInfo>, IpcError> {
    let presets = mango_core::ken_burns::list_presets()
        .iter()
        .map(|p| KenBurnsPresetInfo {
            id: p.id.to_string(),
            label: p.label.to_string(),
            description: p.description.to_string(),
        })
        .collect();
    Ok(presets)
}

#[tauri::command]
#[specta::specta]
pub async fn list_transition_presets() -> Result<Vec<TransitionPresetInfo>, IpcError> {
    let presets = mango_core::transitions::list_presets()
        .iter()
        .map(|p| TransitionPresetInfo {
            id: p.id.to_string(),
            label: p.label.to_string(),
            xfade_name: p.xfade_name.to_string(),
        })
        .collect();
    Ok(presets)
}

#[tauri::command]
#[specta::specta]
pub async fn list_sticker_presets() -> Result<Vec<StickerPresetInfo>, IpcError> {
    let presets = mango_core::stickers::list_presets()
        .iter()
        .map(|p| StickerPresetInfo {
            id: p.id.to_string(),
            label: p.label.to_string(),
            category: p.category.to_string(),
            filename: p.filename.to_string(),
        })
        .collect();
    Ok(presets)
}

#[tauri::command]
#[specta::specta]
pub async fn list_color_effect_presets() -> Result<Vec<ColorEffectPresetInfo>, IpcError> {
    let presets = mango_core::color_presets::list_presets()
        .iter()
        .map(|p| ColorEffectPresetInfo {
            id: p.id.to_string(),
            label: p.label.to_string(),
        })
        .collect();
    Ok(presets)
}
