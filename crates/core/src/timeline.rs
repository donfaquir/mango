use rusqlite::Connection;

use crate::db::queries::{asset, shot_audio, timeline_item, timeline_track, video_clip};
use crate::error::Result;
use crate::ffmpeg::sidecar::FfmpegConfig;
use crate::models::shot_audio::AudioRole;
use crate::models::timeline::{CreateTimelineItemInput, ItemType, TimelineItem};

pub fn import_video_from_clips(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &std::path::Path,
    config: &FfmpegConfig,
) -> Result<Vec<TimelineItem>> {
    let tracks = timeline_track::list_by_episode(conn, episode_id)?;
    let video_track = match tracks.iter().find(|t| t.label == "视频") {
        Some(t) => t,
        None => return Ok(vec![]),
    };

    let clips = video_clip::list(conn, episode_id)?;
    if clips.is_empty() {
        return Ok(vec![]);
    }

    let mut inputs: Vec<CreateTimelineItemInput> = Vec::with_capacity(clips.len());
    let mut timeline_offset: i64 = 0;

    for clip in &clips {
        let a = asset::get_by_id(conn, &clip.source_asset_id)?;
        let abs_path = workspace_root.join(&a.file_path);
        let meta = crate::ffmpeg::probe::probe_video(config, &abs_path)?;
        let start = clip.trim_start_ms.unwrap_or(0);
        let end = clip.trim_end_ms.unwrap_or(meta.duration_ms);
        let duration = end - start;

        inputs.push(CreateTimelineItemInput {
            track_id: video_track.id.clone(),
            asset_id: Some(clip.source_asset_id.clone()),
            item_type: ItemType::Clip,
            position_ms: timeline_offset,
            duration_ms: duration,
            in_point_ms: Some(start),
            out_point_ms: end,
            params_json: Some("{}".into()),
        });

        timeline_offset += duration;
    }

    timeline_item::batch_create(conn, inputs)
}

pub fn import_audio_from_shots(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &std::path::Path,
    config: &FfmpegConfig,
) -> Result<Vec<TimelineItem>> {
    let tracks = timeline_track::list_by_episode(conn, episode_id)?;
    let voice_track = tracks.iter().find(|t| t.label == "配音");
    let bgm_track = tracks.iter().find(|t| t.label == "BGM");
    let sfx_track = tracks.iter().find(|t| t.label == "音效");

    let clips = video_clip::list(conn, episode_id)?;
    if clips.is_empty() {
        return Ok(vec![]);
    }

    let mut clip_durations: Vec<i64> = Vec::with_capacity(clips.len());
    for clip in &clips {
        let a = asset::get_by_id(conn, &clip.source_asset_id)?;
        let abs_path = workspace_root.join(&a.file_path);
        let meta = crate::ffmpeg::probe::probe_video(config, &abs_path)?;
        let start = clip.trim_start_ms.unwrap_or(0);
        let end = clip.trim_end_ms.unwrap_or(meta.duration_ms);
        clip_durations.push(end - start);
    }

    let mut inputs: Vec<CreateTimelineItemInput> = Vec::new();
    let mut timeline_offset: i64 = 0;

    for (i, vc) in clips.iter().enumerate() {
        let source_asset = asset::get_by_id(conn, &vc.source_asset_id)?;
        let shot_id = match source_asset.shot_id {
            Some(ref id) => id.clone(),
            None => {
                timeline_offset += clip_durations[i];
                continue;
            }
        };

        let bindings = shot_audio::list_by_shot(conn, &shot_id)?;
        for binding in &bindings {
            let track_id = match binding.audio_role {
                AudioRole::Voice => voice_track.as_ref().map(|t| &t.id),
                AudioRole::Bgm => bgm_track.as_ref().map(|t| &t.id),
                AudioRole::Sfx => sfx_track.as_ref().map(|t| &t.id),
            };
            let track_id = match track_id {
                Some(id) => id.clone(),
                None => continue,
            };

            let audio_asset = asset::get_by_id(conn, &binding.asset_id)?;
            let audio_path = workspace_root.join(&audio_asset.file_path);
            if !audio_path.exists() {
                tracing::warn!("skipping audio import {}: file not found", binding.id);
                continue;
            }

            let audio_dur = crate::ffmpeg::audio::probe_audio_duration(config, &audio_path)
                .unwrap_or(3000);

            let position = timeline_offset + binding.offset_ms;
            inputs.push(CreateTimelineItemInput {
                track_id,
                asset_id: Some(binding.asset_id.clone()),
                item_type: ItemType::Clip,
                position_ms: position,
                duration_ms: audio_dur,
                in_point_ms: Some(0),
                out_point_ms: audio_dur,
                params_json: Some("{}".into()),
            });
        }

        timeline_offset += clip_durations[i];
    }

    if inputs.is_empty() {
        return Ok(vec![]);
    }

    timeline_item::batch_create(conn, inputs)
}
