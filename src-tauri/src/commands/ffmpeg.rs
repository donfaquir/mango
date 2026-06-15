use std::path::{Path, PathBuf};

use tauri::AppHandle;
use tauri_specta::Event;

use crate::error::IpcError;
use crate::events::FfmpegProgressTick;
use mango_core::ffmpeg;
use mango_core::ffmpeg::FfmpegProgress;

fn progress_emitter(app: &AppHandle) -> impl FnMut(FfmpegProgress) + '_ {
    move |p: FfmpegProgress| {
        let _ = FfmpegProgressTick {
            progress_pct: p.progress_pct,
            current_time_ms: p.current_time_ms,
            total_duration_ms: p.total_duration_ms,
            speed: p.speed,
        }
        .emit(app);
    }
}

#[tauri::command]
#[specta::specta]
pub async fn check_ffmpeg() -> Result<ffmpeg::FfmpegStatus, IpcError> {
    tokio::task::spawn_blocking(|| {
        let config = ffmpeg::FfmpegConfig::from_env();
        Ok(ffmpeg::check_ffmpeg(&config))
    })
    .await
    .map_err(|e| IpcError::internal(format!("task join error: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn probe_video(path: String) -> Result<ffmpeg::VideoMetadata, IpcError> {
    tokio::task::spawn_blocking(move || {
        let config = ffmpeg::FfmpegConfig::from_env();
        ffmpeg::probe_video(&config, Path::new(&path)).map_err(IpcError::from)
    })
    .await
    .map_err(|e| IpcError::internal(format!("task join error: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn trim_video(
    app: AppHandle,
    input: String,
    start_ms: i32,
    end_ms: i32,
    output: String,
    mode: ffmpeg::TrimMode,
) -> Result<String, IpcError> {
    tokio::task::spawn_blocking(move || {
        let config = ffmpeg::FfmpegConfig::from_env();
        let mut cb = progress_emitter(&app);
        let result = ffmpeg::trim_video(
            &config,
            Path::new(&input),
            start_ms as i64,
            end_ms as i64,
            Path::new(&output),
            &mode,
            Some(&mut cb),
        )
        .map_err(IpcError::from)?;
        Ok(result.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| IpcError::internal(format!("task join error: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn split_video(
    app: AppHandle,
    input: String,
    split_points_ms: Vec<i32>,
    output_dir: String,
    mode: ffmpeg::TrimMode,
) -> Result<Vec<String>, IpcError> {
    tokio::task::spawn_blocking(move || {
        let config = ffmpeg::FfmpegConfig::from_env();
        let points: Vec<i64> = split_points_ms.iter().map(|&v| v as i64).collect();
        let mut cb = progress_emitter(&app);
        let results = ffmpeg::split_video(
            &config,
            Path::new(&input),
            &points,
            Path::new(&output_dir),
            &mode,
            Some(&mut cb),
        )
        .map_err(IpcError::from)?;
        Ok(results
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect())
    })
    .await
    .map_err(|e| IpcError::internal(format!("task join error: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn concat_videos(
    app: AppHandle,
    inputs: Vec<String>,
    output: String,
) -> Result<String, IpcError> {
    tokio::task::spawn_blocking(move || {
        let config = ffmpeg::FfmpegConfig::from_env();
        let input_paths: Vec<PathBuf> = inputs.iter().map(PathBuf::from).collect();
        let mut cb = progress_emitter(&app);
        let result =
            ffmpeg::concat_videos(&config, &input_paths, Path::new(&output), Some(&mut cb))
                .map_err(IpcError::from)?;
        Ok(result.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| IpcError::internal(format!("task join error: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn extract_thumbnail(
    input: String,
    timestamp_ms: i32,
    output: String,
) -> Result<String, IpcError> {
    tokio::task::spawn_blocking(move || {
        let config = ffmpeg::FfmpegConfig::from_env();
        let result = ffmpeg::extract_thumbnail(
            &config,
            Path::new(&input),
            timestamp_ms as i64,
            Path::new(&output),
        )
        .map_err(IpcError::from)?;
        Ok(result.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| IpcError::internal(format!("task join error: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn extract_thumbnail_strip(
    state: tauri::State<'_, crate::state::AppState>,
    input: String,
    interval_ms: i32,
    thumb_width: u32,
) -> Result<ffmpeg::ThumbnailStripResult, IpcError> {
    let mounted = super::require_mount(&state)?;
    let workspace_root = mounted.workspace_root.clone();

    tokio::task::spawn_blocking(move || {
        let config = ffmpeg::FfmpegConfig::from_env();
        let input_path = Path::new(&input);
        let cache_key =
            ffmpeg::thumbnail_strip_cache_key(input_path, interval_ms as i64, thumb_width);
        let output_dir = workspace_root.join("thumbnails").join("strips").join(&cache_key);

        ffmpeg::extract_thumbnail_strip(
            &config,
            input_path,
            interval_ms as i64,
            thumb_width,
            &output_dir,
        )
        .map_err(IpcError::from)
    })
    .await
    .map_err(|e| IpcError::internal(format!("task join error: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn extract_waveform(
    state: tauri::State<'_, crate::state::AppState>,
    input: String,
    height: u32,
    px_per_sec: u32,
) -> Result<ffmpeg::WaveformResult, IpcError> {
    let mounted = super::require_mount(&state)?;
    let workspace_root = mounted.workspace_root.clone();

    tokio::task::spawn_blocking(move || {
        let config = ffmpeg::FfmpegConfig::from_env();
        let input_path = Path::new(&input);
        let cache_key = ffmpeg::waveform_cache_key(input_path, height, px_per_sec);
        let output_dir = workspace_root
            .join("thumbnails")
            .join("waveforms")
            .join(&cache_key);

        ffmpeg::extract_waveform(&config, input_path, height, px_per_sec, &output_dir)
            .map_err(IpcError::from)
    })
    .await
    .map_err(|e| IpcError::internal(format!("task join error: {e}")))?
}

#[tauri::command]
#[specta::specta]
pub async fn probe_audio_duration(path: String) -> Result<AudioDuration, IpcError> {
    tokio::task::spawn_blocking(move || {
        let config = ffmpeg::FfmpegConfig::from_env();
        let ms = ffmpeg::probe_audio_duration(&config, std::path::Path::new(&path))
            .map_err(IpcError::from)?;
        Ok(AudioDuration { duration_ms: ms })
    })
    .await
    .map_err(|e| IpcError::internal(format!("task join error: {e}")))?
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioDuration {
    #[specta(type = specta_typescript::Number)]
    pub duration_ms: i64,
}

#[tauri::command]
#[specta::specta]
pub async fn check_audio_alignment(
    video_path: String,
    audio_path: String,
) -> Result<AlignmentInfo, IpcError> {
    tokio::task::spawn_blocking(move || {
        let config = ffmpeg::FfmpegConfig::from_env();
        let video_ms = ffmpeg::probe_video(&config, std::path::Path::new(&video_path))
            .map_err(IpcError::from)?
            .duration_ms;
        let audio_ms = ffmpeg::probe_audio_duration(&config, std::path::Path::new(&audio_path))
            .map_err(IpcError::from)?;
        let strategy = ffmpeg::calculate_alignment_default(video_ms, audio_ms);
        let (strategy_name, diff_ms, suggested_speed) = match &strategy {
            ffmpeg::AlignmentStrategy::Exact => ("exact", 0i64, None),
            ffmpeg::AlignmentStrategy::PadSilence { gap_ms } => ("pad_silence", -*gap_ms, None),
            ffmpeg::AlignmentStrategy::TrimAudio { excess_ms } => ("trim_audio", *excess_ms, None),
            ffmpeg::AlignmentStrategy::RegenerateNeeded { excess_ms, suggested_speed } => {
                ("regenerate_needed", *excess_ms, Some(*suggested_speed))
            }
        };
        Ok(AlignmentInfo {
            video_duration_ms: video_ms,
            audio_duration_ms: audio_ms,
            strategy: strategy_name.to_string(),
            diff_ms,
            suggested_speed,
        })
    })
    .await
    .map_err(|e| IpcError::internal(format!("task join error: {e}")))?
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentInfo {
    #[specta(type = specta_typescript::Number)]
    pub video_duration_ms: i64,
    #[specta(type = specta_typescript::Number)]
    pub audio_duration_ms: i64,
    pub strategy: String,
    #[specta(type = specta_typescript::Number)]
    pub diff_ms: i64,
    pub suggested_speed: Option<f64>,
}
