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
    ffmpeg::probe_video(&config, std::path::Path::new(&path)).map_err(IpcError::from)
}
