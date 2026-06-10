use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::db::queries::{asset, shot_audio, video_clip};
use crate::error::{CoreError, Result};
use crate::ffmpeg::audio::AudioMixInput;
use crate::ffmpeg::commands::TrimMode;
use crate::ffmpeg::progress::FfmpegProgress;
use crate::ffmpeg::sidecar::FfmpegConfig;
use crate::models::shot_audio::AudioRole;

#[derive(Debug)]
pub struct ResolvedClip {
    pub source_path: PathBuf,
    pub trim_start_ms: Option<i64>,
    pub trim_end_ms: Option<i64>,
}

pub fn resolve_clips(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
) -> Result<Vec<ResolvedClip>> {
    let clips = video_clip::list(conn, episode_id)?;
    if clips.is_empty() {
        return Err(CoreError::Validation(
            "no video clips to export for this episode".into(),
        ));
    }

    clips
        .into_iter()
        .map(|clip| {
            let a = asset::get_by_id(conn, &clip.source_asset_id)?;
            let abs_path = workspace_root.join(&a.file_path);
            if !abs_path.exists() {
                return Err(CoreError::Validation(format!(
                    "asset file not found: {}",
                    abs_path.display()
                )));
            }
            Ok(ResolvedClip {
                source_path: abs_path,
                trim_start_ms: clip.trim_start_ms,
                trim_end_ms: clip.trim_end_ms,
            })
        })
        .collect()
}

pub fn export_resolved_clips(
    config: &FfmpegConfig,
    clips: &[ResolvedClip],
    output_path: &Path,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf> {
    if clips.is_empty() {
        return Err(CoreError::Validation(
            "no video clips to export".into(),
        ));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if clips.len() == 1 {
        let clip = &clips[0];
        let no_trim = clip.trim_start_ms.is_none() && clip.trim_end_ms.is_none();
        if no_trim {
            std::fs::copy(&clip.source_path, output_path)?;
        } else {
            let start = clip.trim_start_ms.unwrap_or(0);
            let end = match clip.trim_end_ms {
                Some(e) => e,
                None => crate::ffmpeg::probe::probe_video(config, &clip.source_path)?.duration_ms,
            };
            crate::ffmpeg::commands::trim_video(
                config,
                &clip.source_path,
                start,
                end,
                output_path,
                &TrimMode::Copy,
                on_progress,
            )?;
        }
        return Ok(output_path.to_path_buf());
    }

    let temp_dir = std::env::temp_dir().join(format!("mango_export_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let mut trimmed_paths: Vec<PathBuf> = Vec::with_capacity(clips.len());
    for (i, clip) in clips.iter().enumerate() {
        let needs_trim = clip.trim_start_ms.is_some() || clip.trim_end_ms.is_some();
        if needs_trim {
            let start = clip.trim_start_ms.unwrap_or(0);
            let end = match clip.trim_end_ms {
                Some(e) => e,
                None => {
                    crate::ffmpeg::probe::probe_video(config, &clip.source_path)?.duration_ms
                }
            };
            let trimmed = temp_dir.join(format!("trim_{i}.mp4"));
            crate::ffmpeg::commands::trim_video(
                config,
                &clip.source_path,
                start,
                end,
                &trimmed,
                &TrimMode::Copy,
                None,
            )?;
            trimmed_paths.push(trimmed);
        } else {
            trimmed_paths.push(clip.source_path.clone());
        }
    }

    crate::ffmpeg::commands::concat_videos(config, &trimmed_paths, output_path, on_progress)?;

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(output_path.to_path_buf())
}

// ─── final export (video + audio) ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FinalExportSettings {
    #[serde(default = "default_true")]
    pub include_voice: bool,
    #[serde(default = "default_true")]
    pub include_sfx: bool,
    #[serde(default = "default_true")]
    pub include_bgm: bool,
}

fn default_true() -> bool {
    true
}

impl Default for FinalExportSettings {
    fn default() -> Self {
        Self {
            include_voice: true,
            include_sfx: true,
            include_bgm: true,
        }
    }
}

#[derive(Debug)]
pub struct ResolvedAudio {
    pub path: PathBuf,
    pub role: AudioRole,
    pub volume: f64,
    pub absolute_offset_ms: i64,
}

#[derive(Debug)]
pub struct ResolvedFinalExport {
    pub clips: Vec<ResolvedClip>,
    pub audio_tracks: Vec<ResolvedAudio>,
    pub total_video_duration_ms: i64,
}

pub fn resolve_final_export(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
    settings: &FinalExportSettings,
    config: &FfmpegConfig,
) -> Result<ResolvedFinalExport> {
    let clips = resolve_clips(conn, episode_id, workspace_root)?;

    let mut clip_durations: Vec<i64> = Vec::with_capacity(clips.len());
    for clip in &clips {
        let meta = crate::ffmpeg::probe::probe_video(config, &clip.source_path)?;
        let start = clip.trim_start_ms.unwrap_or(0);
        let end = clip.trim_end_ms.unwrap_or(meta.duration_ms);
        clip_durations.push(end - start);
    }
    let total_video_duration_ms: i64 = clip_durations.iter().sum();

    let video_clips = video_clip::list(conn, episode_id)?;

    let mut audio_tracks: Vec<ResolvedAudio> = Vec::new();
    let mut timeline_offset: i64 = 0;

    for (i, vc) in video_clips.iter().enumerate() {
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
            let include = match binding.audio_role {
                AudioRole::Voice => settings.include_voice,
                AudioRole::Sfx => settings.include_sfx,
                AudioRole::Bgm => settings.include_bgm,
            };
            if !include {
                continue;
            }

            let audio_asset = asset::get_by_id(conn, &binding.asset_id)?;
            let audio_path = workspace_root.join(&audio_asset.file_path);
            if !audio_path.exists() {
                tracing::warn!(
                    "skipping shot_audio {}: file not found at {}",
                    binding.id,
                    audio_path.display()
                );
                continue;
            }

            audio_tracks.push(ResolvedAudio {
                path: audio_path,
                role: binding.audio_role,
                volume: binding.volume,
                absolute_offset_ms: timeline_offset + binding.offset_ms,
            });
        }

        timeline_offset += clip_durations[i];
    }

    Ok(ResolvedFinalExport {
        clips,
        audio_tracks,
        total_video_duration_ms,
    })
}

pub fn export_final(
    config: &FfmpegConfig,
    resolved: &ResolvedFinalExport,
    output_path: &Path,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf> {
    if resolved.clips.is_empty() {
        return Err(CoreError::Validation("no video clips to export".into()));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if resolved.audio_tracks.is_empty() {
        return export_resolved_clips(config, &resolved.clips, output_path, on_progress);
    }

    let temp_dir = std::env::temp_dir().join(format!("mango_final_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let video_only = temp_dir.join("video_assembled.mp4");
    export_resolved_clips(config, &resolved.clips, &video_only, None)?;

    let audio_inputs: Vec<AudioMixInput> = resolved
        .audio_tracks
        .iter()
        .map(|t| AudioMixInput {
            path: t.path.clone(),
            volume: t.volume,
            offset_ms: t.absolute_offset_ms,
        })
        .collect();

    crate::ffmpeg::audio::overlay_audio_on_video(
        config,
        &video_only,
        &audio_inputs,
        output_path,
        on_progress,
    )?;

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(output_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::video_clip;
    use crate::models::video_clip::CreateVideoClipInput;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup() -> Connection {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        conn.execute(
            "INSERT INTO project (id, name) VALUES ('p1', 'Test Project')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO episode (id, project_id, title, order_index) VALUES ('e1', 'p1', 'Ep1', 0)",
            [],
        )
        .unwrap();
        conn
    }

    fn insert_asset(conn: &Connection, id: &str, file_path: &str) {
        conn.execute(
            "INSERT INTO asset (id, project_id, asset_type, file_path, file_size, original_name)
             VALUES (?1, 'p1', 'video', ?2, 1000, 'v.mp4')",
            rusqlite::params![id, file_path],
        )
        .unwrap();
    }

    #[test]
    fn resolve_no_clips_returns_error() {
        let conn = setup();
        let err = resolve_clips(&conn, "e1", Path::new("/tmp")).unwrap_err();
        assert!(
            err.to_string().contains("no video clips"),
            "expected validation error, got: {err}"
        );
    }

    #[test]
    fn resolve_clips_missing_file_returns_error() {
        let conn = setup();
        insert_asset(&conn, "a1", "assets/a1.mp4");
        video_clip::create(
            &conn,
            CreateVideoClipInput {
                project_id: "p1".into(),
                episode_id: Some("e1".into()),
                source_asset_id: "a1".into(),
                label: None,
                trim_start_ms: None,
                trim_end_ms: None,
            },
        )
        .unwrap();

        let err = resolve_clips(&conn, "e1", Path::new("/nonexistent")).unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "expected file-not-found error, got: {err}"
        );
    }

    #[test]
    fn resolve_clips_with_real_files() {
        let tmp = TempDir::new().unwrap();
        let video_path = "assets/v1.mp4";
        let abs = tmp.path().join(video_path);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        std::fs::write(&abs, b"fake video content").unwrap();

        let conn = setup();
        insert_asset(&conn, "a1", video_path);
        video_clip::create(
            &conn,
            CreateVideoClipInput {
                project_id: "p1".into(),
                episode_id: Some("e1".into()),
                source_asset_id: "a1".into(),
                label: None,
                trim_start_ms: Some(1000),
                trim_end_ms: Some(5000),
            },
        )
        .unwrap();

        let clips = resolve_clips(&conn, "e1", tmp.path()).unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].source_path, abs);
        assert_eq!(clips[0].trim_start_ms, Some(1000));
        assert_eq!(clips[0].trim_end_ms, Some(5000));
    }

    #[test]
    fn export_empty_clips_returns_error() {
        let config = FfmpegConfig {
            ffmpeg_path: None,
            ffprobe_path: None,
        };
        let err =
            export_resolved_clips(&config, &[], Path::new("/tmp/out.mp4"), None).unwrap_err();
        assert!(
            err.to_string().contains("no video clips"),
            "expected validation error, got: {err}"
        );
    }

    #[test]
    fn export_single_no_trim_copies_file() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("source.mp4");
        let content = b"fake mp4 data for testing copy path";
        std::fs::write(&src, content).unwrap();

        let output = tmp.path().join("output.mp4");
        let config = FfmpegConfig {
            ffmpeg_path: None,
            ffprobe_path: None,
        };

        let clip = ResolvedClip {
            source_path: src.clone(),
            trim_start_ms: None,
            trim_end_ms: None,
        };

        let result = export_resolved_clips(&config, &[clip], &output, None).unwrap();
        assert_eq!(result, output);
        assert_eq!(std::fs::read(&output).unwrap(), content);
    }
}
