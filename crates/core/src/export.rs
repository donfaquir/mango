use std::path::{Path, PathBuf};

use rusqlite::Connection;
use uuid::Uuid;

use crate::db::queries::{asset, video_clip};
use crate::error::{CoreError, Result};
use crate::ffmpeg::commands::TrimMode;
use crate::ffmpeg::progress::FfmpegProgress;
use crate::ffmpeg::sidecar::FfmpegConfig;

pub fn export_video_clips(
    conn: &Connection,
    config: &FfmpegConfig,
    episode_id: &str,
    workspace_root: &Path,
    output_path: &Path,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf> {
    let clips = video_clip::list(conn, episode_id)?;
    if clips.is_empty() {
        return Err(CoreError::Validation(
            "no video clips to export for this episode".into(),
        ));
    }

    let resolved: Vec<(i64, i64, PathBuf)> = clips
        .iter()
        .map(|clip| {
            let a = asset::get_by_id(conn, &clip.source_asset_id)?;
            let abs_path = workspace_root.join(&a.file_path);
            if !abs_path.exists() {
                return Err(CoreError::Validation(format!(
                    "asset file not found: {}",
                    abs_path.display()
                )));
            }
            let start = clip.trim_start_ms.unwrap_or(0);
            let end = clip.trim_end_ms.unwrap_or_else(|| {
                crate::ffmpeg::probe::probe_video(config, &abs_path)
                    .map(|m| m.duration_ms)
                    .unwrap_or(0)
            });
            Ok((start, end, abs_path))
        })
        .collect::<Result<Vec<_>>>()?;

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if resolved.len() == 1 {
        let (start, end, ref src_path) = resolved[0];
        let no_trim = clips[0].trim_start_ms.is_none() && clips[0].trim_end_ms.is_none();
        if no_trim || (start == 0 && end == 0) {
            std::fs::copy(src_path, output_path)?;
        } else {
            crate::ffmpeg::commands::trim_video(
                config, src_path, start, end, output_path, &TrimMode::Copy, on_progress,
            )?;
        }
        return Ok(output_path.to_path_buf());
    }

    // Multi-clip: trim each to temp, then concat
    let temp_dir = std::env::temp_dir().join(format!("mango_export_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let mut trimmed_paths: Vec<PathBuf> = Vec::with_capacity(resolved.len());
    for (i, (start, end, src_path)) in resolved.iter().enumerate() {
        let needs_trim = clips[i].trim_start_ms.is_some() || clips[i].trim_end_ms.is_some();
        if needs_trim {
            let trimmed = temp_dir.join(format!("trim_{i}.mp4"));
            crate::ffmpeg::commands::trim_video(
                config, src_path, *start, *end, &trimmed, &TrimMode::Copy, None,
            )?;
            trimmed_paths.push(trimmed);
        } else {
            trimmed_paths.push(src_path.clone());
        }
    }

    crate::ffmpeg::commands::concat_videos(config, &trimmed_paths, output_path, on_progress)?;

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(output_path.to_path_buf())
}
