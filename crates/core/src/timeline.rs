use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::db::queries::{asset, episode, project, shot, shot_audio, timeline_item, timeline_track};
use crate::error::Result;
use crate::ffmpeg::sidecar::FfmpegConfig;
use crate::models::shot::{ListShotsOptions, Shot};
use crate::models::shot_audio::AudioRole;
use crate::models::timeline::{CreateTimelineItemInput, ItemType, TimelineItem};

fn resolve_project_root(conn: &Connection, episode_id: &str, workspace_root: &Path) -> Result<PathBuf> {
    let ep = episode::get_by_id(conn, episode_id)?;
    let proj = project::get_by_id(conn, &ep.project_id)?;
    let root = Path::new(&proj.root_path);
    if root.is_absolute() {
        Ok(root.to_path_buf())
    } else {
        Ok(workspace_root.join(root))
    }
}

fn shots_with_video(conn: &Connection, episode_id: &str) -> Result<Vec<Shot>> {
    let all = shot::list(conn, ListShotsOptions { episode_id: episode_id.to_string() })?;
    Ok(all.into_iter().filter(|s| s.adopted_asset_id.is_some()).collect())
}

pub fn import_video_from_clips(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
    config: &FfmpegConfig,
) -> Result<Vec<TimelineItem>> {
    let project_root = resolve_project_root(conn, episode_id, workspace_root)?;

    let tracks = timeline_track::list_by_episode(conn, episode_id)?;
    let video_track = match tracks.iter().find(|t| t.label == "视频") {
        Some(t) => t,
        None => return Ok(vec![]),
    };

    let shots = shots_with_video(conn, episode_id)?;
    if shots.is_empty() {
        return Ok(vec![]);
    }

    let mut inputs: Vec<CreateTimelineItemInput> = Vec::with_capacity(shots.len());
    let mut timeline_offset: i64 = 0;

    for s in &shots {
        let asset_id = s.adopted_asset_id.as_ref().unwrap();
        let a = asset::get_by_id(conn, asset_id)?;
        let abs_path = project_root.join(&a.file_path);
        if !abs_path.exists() {
            tracing::warn!(
                "skipping video import for shot {}: file not found: {}",
                s.id, abs_path.display()
            );
            continue;
        }
        let meta = crate::ffmpeg::probe::probe_video(config, &abs_path)?;
        let duration = meta.duration_ms;

        inputs.push(CreateTimelineItemInput {
            track_id: video_track.id.clone(),
            asset_id: Some(asset_id.clone()),
            item_type: ItemType::Clip,
            position_ms: timeline_offset,
            duration_ms: duration,
            in_point_ms: Some(0),
            out_point_ms: duration,
            params_json: Some("{}".into()),
        });

        timeline_offset += duration;
    }

    timeline_item::batch_create(conn, inputs)
}

pub fn import_audio_from_shots(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
    config: &FfmpegConfig,
) -> Result<Vec<TimelineItem>> {
    let project_root = resolve_project_root(conn, episode_id, workspace_root)?;

    let tracks = timeline_track::list_by_episode(conn, episode_id)?;
    let voice_track = tracks.iter().find(|t| t.label == "配音");
    let bgm_track = tracks.iter().find(|t| t.label == "BGM");
    let sfx_track = tracks.iter().find(|t| t.label == "音效");

    let shots = shots_with_video(conn, episode_id)?;
    if shots.is_empty() {
        return Ok(vec![]);
    }

    let mut shot_durations: Vec<i64> = Vec::with_capacity(shots.len());
    for s in &shots {
        let asset_id = s.adopted_asset_id.as_ref().unwrap();
        let a = asset::get_by_id(conn, asset_id)?;
        let abs_path = project_root.join(&a.file_path);
        if !abs_path.exists() {
            shot_durations.push(0);
            continue;
        }
        let meta = crate::ffmpeg::probe::probe_video(config, &abs_path)?;
        shot_durations.push(meta.duration_ms);
    }

    let mut inputs: Vec<CreateTimelineItemInput> = Vec::new();
    let mut timeline_offset: i64 = 0;

    for (i, s) in shots.iter().enumerate() {
        let bindings = shot_audio::list_by_shot(conn, &s.id)?;
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
            let audio_path = project_root.join(&audio_asset.file_path);
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

        timeline_offset += shot_durations[i];
    }

    if inputs.is_empty() {
        return Ok(vec![]);
    }

    timeline_item::batch_create(conn, inputs)
}
