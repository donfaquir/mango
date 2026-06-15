use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

use crate::error::{CoreError, Result};

use super::sidecar::FfmpegConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct WaveformResult {
    pub path: String,
    pub width: u32,
    pub height: u32,
}

pub fn extract_waveform(
    config: &FfmpegConfig,
    input: &Path,
    height: u32,
    px_per_sec: u32,
    output_dir: &Path,
) -> Result<WaveformResult> {
    if height == 0 {
        return Err(CoreError::Validation("height must be > 0".into()));
    }
    if px_per_sec == 0 {
        return Err(CoreError::Validation("px_per_sec must be > 0".into()));
    }
    if !input.exists() {
        return Err(CoreError::Validation(format!(
            "input file does not exist: {}",
            input.display()
        )));
    }

    let output_path = output_dir.join("waveform.png");
    if output_path.exists() {
        let (w, h) = read_png_dimensions(&output_path).unwrap_or((0, height));
        return Ok(WaveformResult {
            path: output_path.to_string_lossy().into_owned(),
            width: w,
            height: h,
        });
    }

    let duration_ms =
        super::audio::probe_audio_duration(config, input).unwrap_or(5000);
    let duration_secs = (duration_ms as f64 / 1000.0).max(0.1);
    let width = ((duration_secs * px_per_sec as f64) as u32).max(100);

    std::fs::create_dir_all(output_dir)?;

    let size = format!("{width}x{height}");
    let filter = format!("showwavespic=s={size}:colors=#93c5fd");

    let mut cmd = Command::new(config.ffmpeg_bin());
    cmd.args(["-y", "-i"])
        .arg(input)
        .args(["-filter_complex", &filter, "-frames:v", "1"])
        .arg(&output_path);

    let output = cmd
        .output()
        .map_err(|e| CoreError::Ffmpeg(format!("failed to execute ffmpeg: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::Ffmpeg(format!(
            "ffmpeg waveform extraction failed: {stderr}"
        )));
    }

    Ok(WaveformResult {
        path: output_path.to_string_lossy().into_owned(),
        width,
        height,
    })
}

pub fn waveform_cache_key(input_path: &Path, height: u32, px_per_sec: u32) -> String {
    use sha2::{Digest, Sha256};
    let path_str = input_path.to_string_lossy();
    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    let hash = hasher.finalize();
    let hash_hex: String = hash.iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("{hash_hex}_{height}_{px_per_sec}")
}

fn read_png_dimensions(path: &Path) -> Option<(u32, u32)> {
    let data = std::fs::read(path).ok()?;
    // PNG header: 8-byte signature, then IHDR chunk (4 len + 4 type + 4 width + 4 height)
    if data.len() < 24 || &data[..4] != b"\x89PNG" {
        return None;
    }
    let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    Some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_deterministic() {
        let k1 = waveform_cache_key(Path::new("/a/b.mp3"), 64, 50);
        let k2 = waveform_cache_key(Path::new("/a/b.mp3"), 64, 50);
        assert_eq!(k1, k2);
    }

    #[test]
    fn cache_key_differs_by_height() {
        let k1 = waveform_cache_key(Path::new("/a/b.mp3"), 64, 50);
        let k2 = waveform_cache_key(Path::new("/a/b.mp3"), 128, 50);
        assert_ne!(k1, k2);
    }

    #[test]
    fn cache_key_differs_by_px_per_sec() {
        let k1 = waveform_cache_key(Path::new("/a/b.mp3"), 64, 50);
        let k2 = waveform_cache_key(Path::new("/a/b.mp3"), 64, 100);
        assert_ne!(k1, k2);
    }

    #[test]
    fn invalid_height_zero() {
        let config = FfmpegConfig {
            ffmpeg_path: None,
            ffprobe_path: None,
        };
        let err = extract_waveform(&config, Path::new("/x"), 0, 50, Path::new("/tmp"));
        assert!(err.unwrap_err().to_string().contains("height must be > 0"));
    }

    #[test]
    fn invalid_px_per_sec_zero() {
        let config = FfmpegConfig {
            ffmpeg_path: None,
            ffprobe_path: None,
        };
        let err = extract_waveform(&config, Path::new("/x"), 64, 0, Path::new("/tmp"));
        assert!(err
            .unwrap_err()
            .to_string()
            .contains("px_per_sec must be > 0"));
    }

    #[test]
    fn nonexistent_input() {
        let config = FfmpegConfig {
            ffmpeg_path: None,
            ffprobe_path: None,
        };
        let err = extract_waveform(
            &config,
            Path::new("/nonexistent/audio.mp3"),
            64,
            50,
            Path::new("/tmp"),
        );
        assert!(err
            .unwrap_err()
            .to_string()
            .contains("input file does not exist"));
    }
}
