use std::path::{Path, PathBuf};

use crate::error::IpcError;
use mango_core::ffmpeg;

#[tauri::command]
#[specta::specta]
pub async fn check_ffmpeg() -> Result<ffmpeg::FfmpegStatus, IpcError> {
    let config = ffmpeg::FfmpegConfig::from_env();
    Ok(ffmpeg::check_ffmpeg(&config))
}

#[tauri::command]
#[specta::specta]
pub async fn probe_video(path: String) -> Result<ffmpeg::VideoMetadata, IpcError> {
    let config = ffmpeg::FfmpegConfig::from_env();
    ffmpeg::probe_video(&config, Path::new(&path)).map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn trim_video(
    input: String,
    start_ms: i32,
    end_ms: i32,
    output: String,
    mode: ffmpeg::TrimMode,
) -> Result<String, IpcError> {
    let config = ffmpeg::FfmpegConfig::from_env();
    let result = ffmpeg::trim_video(
        &config,
        Path::new(&input),
        start_ms as i64,
        end_ms as i64,
        Path::new(&output),
        &mode,
    )
    .map_err(IpcError::from)?;
    Ok(result.to_string_lossy().into_owned())
}

#[tauri::command]
#[specta::specta]
pub async fn split_video(
    input: String,
    split_points_ms: Vec<i32>,
    output_dir: String,
    mode: ffmpeg::TrimMode,
) -> Result<Vec<String>, IpcError> {
    let config = ffmpeg::FfmpegConfig::from_env();
    let points: Vec<i64> = split_points_ms.iter().map(|&v| v as i64).collect();
    let results = ffmpeg::split_video(
        &config,
        Path::new(&input),
        &points,
        Path::new(&output_dir),
        &mode,
    )
    .map_err(IpcError::from)?;
    Ok(results
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn concat_videos(inputs: Vec<String>, output: String) -> Result<String, IpcError> {
    let config = ffmpeg::FfmpegConfig::from_env();
    let input_paths: Vec<PathBuf> = inputs.iter().map(PathBuf::from).collect();
    let result =
        ffmpeg::concat_videos(&config, &input_paths, Path::new(&output)).map_err(IpcError::from)?;
    Ok(result.to_string_lossy().into_owned())
}

#[tauri::command]
#[specta::specta]
pub async fn extract_thumbnail(
    input: String,
    timestamp_ms: i32,
    output: String,
) -> Result<String, IpcError> {
    let config = ffmpeg::FfmpegConfig::from_env();
    let result = ffmpeg::extract_thumbnail(
        &config,
        Path::new(&input),
        timestamp_ms as i64,
        Path::new(&output),
    )
    .map_err(IpcError::from)?;
    Ok(result.to_string_lossy().into_owned())
}
