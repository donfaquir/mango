use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

use crate::error::{CoreError, Result};

use super::probe::VideoMetadata;
use super::sidecar::FfmpegConfig;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub enum TrimMode {
    Copy,
    Reencode,
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

pub fn ms_to_ffmpeg_time(ms: i64) -> String {
    let ms = ms.max(0);
    let total_secs = ms / 1000;
    let millis = ms % 1000;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}.{millis:03}")
}

pub fn has_encoder(config: &FfmpegConfig, encoder: &str) -> bool {
    Command::new(config.ffmpeg_bin())
        .args(["-encoders", "-hide_banner"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.lines().any(|line| {
                // encoder lines look like: " V..... libx264  ..."
                line.split_whitespace()
                    .nth(1)
                    .is_some_and(|name| name == encoder)
            })
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Trim
// ---------------------------------------------------------------------------

pub fn trim_video(config: &FfmpegConfig, input: &Path, start_ms: i64, end_ms: i64, output: &Path, mode: &TrimMode) -> Result<PathBuf> {
    validate_trim_params(config, input, start_ms, end_ms, output)?;

    let start_t = ms_to_ffmpeg_time(start_ms);
    let end_t = ms_to_ffmpeg_time(end_ms);

    let mut cmd = Command::new(config.ffmpeg_bin());
    cmd.arg("-y");

    match mode {
        TrimMode::Copy => {
            cmd.args(["-ss", &start_t, "-to", &end_t])
                .arg("-i").arg(input)
                .args(["-c", "copy"]);
        }
        TrimMode::Reencode => {
            cmd.arg("-i").arg(input)
                .args(["-ss", &start_t, "-to", &end_t]);

            let meta = super::probe::probe_video(config, input)?;
            let use_libx264 = has_encoder(config, "libx264");

            if use_libx264 {
                cmd.args(["-c:v", "libx264", "-preset", "fast", "-crf", "18"]);
            }

            if meta.audio_codec.is_some() {
                cmd.args(["-c:a", "aac"]);
            } else {
                cmd.arg("-an");
            }
        }
    }

    cmd.arg(output);
    run_ffmpeg(&mut cmd)?;
    Ok(output.to_path_buf())
}

fn validate_trim_params(config: &FfmpegConfig, input: &Path, start_ms: i64, end_ms: i64, output: &Path) -> Result<()> {
    if start_ms < 0 {
        return Err(CoreError::Validation("start_ms must be >= 0".into()));
    }
    if end_ms <= start_ms {
        return Err(CoreError::Validation("end_ms must be > start_ms".into()));
    }
    if !input.exists() {
        return Err(CoreError::Validation(format!("input file does not exist: {}", input.display())));
    }
    if let Some(parent) = output.parent() && !parent.exists() {
        return Err(CoreError::Validation(format!(
            "output directory does not exist: {}",
            parent.display()
        )));
    }
    let meta = super::probe::probe_video(config, input)?;
    if end_ms > meta.duration_ms {
        return Err(CoreError::Validation(format!(
            "end_ms ({end_ms}) exceeds video duration ({})",
            meta.duration_ms
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Split
// ---------------------------------------------------------------------------

pub fn split_video(config: &FfmpegConfig, input: &Path, split_points_ms: &[i64], output_dir: &Path, mode: &TrimMode) -> Result<Vec<PathBuf>> {
    validate_split_params(config, input, split_points_ms, output_dir)?;

    let meta = super::probe::probe_video(config, input)?;
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("output");
    let ext = input.extension().and_then(|s| s.to_str()).unwrap_or("mp4");

    let mut boundaries: Vec<i64> = Vec::with_capacity(split_points_ms.len() + 2);
    boundaries.push(0);
    boundaries.extend_from_slice(split_points_ms);
    boundaries.push(meta.duration_ms);

    let mut outputs = Vec::new();
    for i in 0..boundaries.len() - 1 {
        let start = boundaries[i];
        let end = boundaries[i + 1];
        let out_path = output_dir.join(format!("{stem}_part{}.{ext}", i + 1));
        trim_video(config, input, start, end, &out_path, mode)?;
        outputs.push(out_path);
    }

    Ok(outputs)
}

fn validate_split_params(config: &FfmpegConfig, input: &Path, split_points_ms: &[i64], output_dir: &Path) -> Result<()> {
    if split_points_ms.is_empty() {
        return Err(CoreError::Validation("split_points_ms must not be empty".into()));
    }
    for (i, &point) in split_points_ms.iter().enumerate() {
        if i > 0 && point <= split_points_ms[i - 1] {
            return Err(CoreError::Validation(
                "split_points_ms must be strictly increasing".into(),
            ));
        }
    }
    if !input.exists() {
        return Err(CoreError::Validation(format!("input file does not exist: {}", input.display())));
    }
    if !output_dir.exists() {
        return Err(CoreError::Validation(format!("output directory does not exist: {}", output_dir.display())));
    }

    let meta = super::probe::probe_video(config, input)?;
    for &point in split_points_ms {
        if point <= 0 || point >= meta.duration_ms {
            return Err(CoreError::Validation(format!(
                "split point {point}ms is out of range (0, {})",
                meta.duration_ms
            )));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Concat
// ---------------------------------------------------------------------------

pub fn concat_videos(config: &FfmpegConfig, inputs: &[PathBuf], output: &Path) -> Result<PathBuf> {
    if inputs.len() < 2 {
        return Err(CoreError::Validation("concat requires at least 2 input files".into()));
    }
    for p in inputs {
        if !p.exists() {
            return Err(CoreError::Validation(format!("input file does not exist: {}", p.display())));
        }
    }
    if let Some(parent) = output.parent() && !parent.exists() {
        return Err(CoreError::Validation(format!(
            "output directory does not exist: {}",
            parent.display()
        )));
    }

    check_concat_compatibility(config, inputs)?;

    let list_file = std::env::temp_dir().join(format!("mango_concat_{}.txt", std::process::id()));
    let list_content: String = inputs
        .iter()
        .map(|p| format!("file '{}'", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&list_file, &list_content)?;

    let mut cmd = Command::new(config.ffmpeg_bin());
    cmd.args(["-y", "-f", "concat", "-safe", "0", "-i"])
        .arg(&list_file)
        .args(["-c", "copy"])
        .arg(output);

    let result = run_ffmpeg(&mut cmd);
    let _ = std::fs::remove_file(&list_file);
    result?;

    Ok(output.to_path_buf())
}

fn check_concat_compatibility(config: &FfmpegConfig, inputs: &[PathBuf]) -> Result<()> {
    let metas: Vec<VideoMetadata> = inputs
        .iter()
        .map(|p| super::probe::probe_video(config, p))
        .collect::<Result<Vec<_>>>()?;

    let first = &metas[0];
    for (i, meta) in metas.iter().enumerate().skip(1) {
        if meta.width != first.width || meta.height != first.height {
            return Err(CoreError::Validation(format!(
                "cannot concat: input #{} has resolution {}x{}, but input #1 has {}x{}. All inputs must have the same resolution.",
                i + 1, meta.width, meta.height, first.width, first.height
            )));
        }
        if (meta.fps - first.fps).abs() > 0.01 {
            return Err(CoreError::Validation(format!(
                "cannot concat: input #{} has fps {:.2}, but input #1 has {:.2}. All inputs must have the same frame rate.",
                i + 1, meta.fps, first.fps
            )));
        }
        if meta.video_codec != first.video_codec {
            return Err(CoreError::Validation(format!(
                "cannot concat: input #{} uses codec '{}', but input #1 uses '{}'. All inputs must use the same video codec.",
                i + 1, meta.video_codec, first.video_codec
            )));
        }
        let has_audio_first = first.audio_codec.is_some();
        let has_audio_current = meta.audio_codec.is_some();
        if has_audio_first != has_audio_current {
            return Err(CoreError::Validation(format!(
                "cannot concat: input #{} {} audio but input #1 {}. All inputs must either have audio or not.",
                i + 1,
                if has_audio_current { "has" } else { "lacks" },
                if has_audio_first { "has" } else { "lacks" }
            )));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Thumbnail
// ---------------------------------------------------------------------------

pub fn extract_thumbnail(config: &FfmpegConfig, input: &Path, timestamp_ms: i64, output: &Path) -> Result<PathBuf> {
    if timestamp_ms < 0 {
        return Err(CoreError::Validation("timestamp_ms must be >= 0".into()));
    }
    if !input.exists() {
        return Err(CoreError::Validation(format!("input file does not exist: {}", input.display())));
    }
    if let Some(parent) = output.parent() && !parent.exists() {
        return Err(CoreError::Validation(format!(
            "output directory does not exist: {}",
            parent.display()
        )));
    }

    let ts = ms_to_ffmpeg_time(timestamp_ms);
    let mut cmd = Command::new(config.ffmpeg_bin());
    cmd.args(["-y", "-ss", &ts])
        .arg("-i").arg(input)
        .args(["-frames:v", "1", "-q:v", "2"])
        .arg(output);

    run_ffmpeg(&mut cmd)?;
    Ok(output.to_path_buf())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_ffmpeg(cmd: &mut Command) -> Result<()> {
    let output = cmd.output().map_err(|e| {
        CoreError::Ffmpeg(format!("failed to execute ffmpeg: {e}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::Ffmpeg(format!("ffmpeg exited with error: {stderr}")));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_format_zero() {
        assert_eq!(ms_to_ffmpeg_time(0), "00:00:00.000");
    }

    #[test]
    fn time_format_millis_only() {
        assert_eq!(ms_to_ffmpeg_time(123), "00:00:00.123");
    }

    #[test]
    fn time_format_seconds() {
        assert_eq!(ms_to_ffmpeg_time(5500), "00:00:05.500");
    }

    #[test]
    fn time_format_minutes() {
        assert_eq!(ms_to_ffmpeg_time(90061), "00:01:30.061");
    }

    #[test]
    fn time_format_hours() {
        assert_eq!(ms_to_ffmpeg_time(3661500), "01:01:01.500");
    }

    #[test]
    fn time_format_negative_clamps_to_zero() {
        assert_eq!(ms_to_ffmpeg_time(-100), "00:00:00.000");
    }

    #[test]
    fn time_format_large_value() {
        assert_eq!(ms_to_ffmpeg_time(86400000), "24:00:00.000");
    }

    #[test]
    fn trim_invalid_start_gte_end() {
        let config = FfmpegConfig { ffmpeg_path: None, ffprobe_path: None };
        let tmp = std::env::temp_dir();
        let err = validate_trim_params(&config, Path::new("/nonexistent"), 5000, 3000, &tmp.join("out.mp4"));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("end_ms must be > start_ms"));
    }

    #[test]
    fn trim_negative_start() {
        let config = FfmpegConfig { ffmpeg_path: None, ffprobe_path: None };
        let tmp = std::env::temp_dir();
        let err = validate_trim_params(&config, Path::new("/nonexistent"), -1, 3000, &tmp.join("out.mp4"));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("start_ms must be >= 0"));
    }

    #[test]
    fn trim_input_not_found() {
        let config = FfmpegConfig { ffmpeg_path: None, ffprobe_path: None };
        let tmp = std::env::temp_dir();
        let err = validate_trim_params(&config, Path::new("/nonexistent.mp4"), 0, 3000, &tmp.join("out.mp4"));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn split_empty_points() {
        let config = FfmpegConfig { ffmpeg_path: None, ffprobe_path: None };
        let err = validate_split_params(&config, Path::new("/nonexistent"), &[], Path::new("/tmp"));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("must not be empty"));
    }

    #[test]
    fn split_non_increasing() {
        let config = FfmpegConfig { ffmpeg_path: None, ffprobe_path: None };
        let err = validate_split_params(&config, Path::new("/nonexistent"), &[5000, 3000], Path::new("/tmp"));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("strictly increasing"));
    }

    #[test]
    fn concat_single_input() {
        let config = FfmpegConfig { ffmpeg_path: None, ffprobe_path: None };
        let err = concat_videos(&config, &[PathBuf::from("/a.mp4")], Path::new("/out.mp4"));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("at least 2"));
    }

    #[test]
    fn concat_missing_input() {
        let config = FfmpegConfig { ffmpeg_path: None, ffprobe_path: None };
        let err = concat_videos(
            &config,
            &[PathBuf::from("/nonexistent1.mp4"), PathBuf::from("/nonexistent2.mp4")],
            Path::new("/out.mp4"),
        );
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn thumbnail_negative_timestamp() {
        let config = FfmpegConfig { ffmpeg_path: None, ffprobe_path: None };
        let err = extract_thumbnail(&config, Path::new("/nonexistent"), -1, Path::new("/out.jpg"));
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("timestamp_ms must be >= 0"));
    }
}
