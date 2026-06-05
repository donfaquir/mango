use std::path::{Path, PathBuf};

use rusqlite::Connection;
use uuid::Uuid;

use crate::db::queries::{asset, video_clip};
use crate::error::{CoreError, Result};
use crate::ffmpeg::commands::TrimMode;
use crate::ffmpeg::progress::FfmpegProgress;
use crate::ffmpeg::sidecar::FfmpegConfig;

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
