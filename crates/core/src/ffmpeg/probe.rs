use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

use crate::error::{CoreError, Result};

use super::sidecar::FfmpegConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct VideoMetadata {
    #[specta(type = specta_typescript::Number)]
    pub duration_ms: i64,
    pub width: u32,
    pub height: u32,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub fps: f64,
    pub bitrate_kbps: Option<u32>,
    #[specta(type = specta_typescript::Number)]
    pub file_size_bytes: u64,
    pub pixel_format: Option<String>,
}

pub fn probe_video(config: &FfmpegConfig, path: &Path) -> Result<VideoMetadata> {
    if !path.exists() {
        return Err(CoreError::Validation(format!(
            "file does not exist: {}",
            path.display()
        )));
    }

    let output = Command::new(config.ffprobe_bin())
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_streams",
            "-show_format",
        ])
        .arg(path)
        .output()
        .map_err(|e| {
            CoreError::Ffmpeg(format!(
                "failed to run ffprobe: {e}. Is FFmpeg installed?"
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::Ffmpeg(format!("ffprobe failed: {stderr}")));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_probe_output(&stdout)
}

fn parse_probe_output(json_str: &str) -> Result<VideoMetadata> {
    let root: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| CoreError::Ffmpeg(format!("invalid ffprobe JSON: {e}")))?;

    let streams = root["streams"]
        .as_array()
        .ok_or_else(|| CoreError::Ffmpeg("ffprobe output missing 'streams' array".into()))?;

    let video_stream = streams
        .iter()
        .find(|s| s["codec_type"].as_str() == Some("video"))
        .ok_or_else(|| CoreError::Ffmpeg("no video stream found".into()))?;

    let audio_stream = streams
        .iter()
        .find(|s| s["codec_type"].as_str() == Some("audio"));

    let format = &root["format"];

    let duration_secs: f64 = format["duration"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| video_stream["duration"].as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0.0);

    let width = video_stream["width"].as_u64().unwrap_or(0) as u32;
    let height = video_stream["height"].as_u64().unwrap_or(0) as u32;
    let video_codec = video_stream["codec_name"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let audio_codec = audio_stream
        .and_then(|s| s["codec_name"].as_str())
        .map(|s| s.to_string());

    let fps = parse_frame_rate(
        video_stream["r_frame_rate"]
            .as_str()
            .unwrap_or("0/1"),
    );

    let bitrate_kbps = format["bit_rate"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .map(|bps| (bps / 1000) as u32);

    let file_size_bytes = format["size"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let pixel_format = video_stream["pix_fmt"]
        .as_str()
        .map(|s| s.to_string());

    Ok(VideoMetadata {
        duration_ms: (duration_secs * 1000.0) as i64,
        width,
        height,
        video_codec,
        audio_codec,
        fps,
        bitrate_kbps,
        file_size_bytes,
        pixel_format,
    })
}

fn parse_frame_rate(rate_str: &str) -> f64 {
    // ffprobe returns frame rate as "30/1" or "30000/1001"
    let parts: Vec<&str> = rate_str.split('/').collect();
    match parts.as_slice() {
        [num, den] => {
            let n: f64 = num.parse().unwrap_or(0.0);
            let d: f64 = den.parse().unwrap_or(1.0);
            if d == 0.0 { 0.0 } else { n / d }
        }
        [num] => num.parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PROBE_OUTPUT: &str = r#"{
        "streams": [
            {
                "codec_name": "h264",
                "codec_type": "video",
                "width": 1920,
                "height": 1080,
                "r_frame_rate": "30/1",
                "duration": "10.500000"
            },
            {
                "codec_name": "aac",
                "codec_type": "audio"
            }
        ],
        "format": {
            "duration": "10.500000",
            "bit_rate": "5000000",
            "size": "6562500"
        }
    }"#;

    #[test]
    fn parse_probe_full() {
        let meta = parse_probe_output(SAMPLE_PROBE_OUTPUT).unwrap();
        assert_eq!(meta.duration_ms, 10500);
        assert_eq!(meta.width, 1920);
        assert_eq!(meta.height, 1080);
        assert_eq!(meta.video_codec, "h264");
        assert_eq!(meta.audio_codec, Some("aac".to_string()));
        assert!((meta.fps - 30.0).abs() < 0.01);
        assert_eq!(meta.bitrate_kbps, Some(5000));
        assert_eq!(meta.file_size_bytes, 6562500);
    }

    #[test]
    fn parse_probe_no_audio() {
        let json = r#"{
            "streams": [
                {
                    "codec_name": "vp9",
                    "codec_type": "video",
                    "width": 1280,
                    "height": 720,
                    "r_frame_rate": "24000/1001"
                }
            ],
            "format": {
                "duration": "5.200000",
                "size": "1300000"
            }
        }"#;
        let meta = parse_probe_output(json).unwrap();
        assert_eq!(meta.duration_ms, 5200);
        assert_eq!(meta.width, 1280);
        assert_eq!(meta.height, 720);
        assert_eq!(meta.video_codec, "vp9");
        assert!(meta.audio_codec.is_none());
        assert!((meta.fps - 23.976).abs() < 0.01);
        assert!(meta.bitrate_kbps.is_none());
    }

    #[test]
    fn parse_probe_no_video_stream() {
        let json = r#"{
            "streams": [
                { "codec_name": "aac", "codec_type": "audio" }
            ],
            "format": { "duration": "3.0" }
        }"#;
        let err = parse_probe_output(json).unwrap_err();
        assert!(err.to_string().contains("no video stream"));
    }

    #[test]
    fn parse_probe_invalid_json() {
        let err = parse_probe_output("not json at all").unwrap_err();
        assert!(err.to_string().contains("invalid ffprobe JSON"));
    }

    #[test]
    fn frame_rate_fraction() {
        assert!((parse_frame_rate("30000/1001") - 29.97).abs() < 0.01);
    }

    #[test]
    fn frame_rate_integer() {
        assert!((parse_frame_rate("25/1") - 25.0).abs() < 0.01);
    }

    #[test]
    fn frame_rate_bare_number() {
        assert!((parse_frame_rate("60") - 60.0).abs() < 0.01);
    }

    #[test]
    fn frame_rate_zero_denominator() {
        assert_eq!(parse_frame_rate("30/0"), 0.0);
    }
}
