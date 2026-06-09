//! FFmpeg audio operations: mixing, overlay, padding, trimming, probing.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

use super::filter_graph::{AmixDuration, FilterGraph};
use super::progress::{run_ffmpeg_with_progress, FfmpegProgress};
use super::sidecar::FfmpegConfig;
use crate::error::{CoreError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct AudioMixInput {
    pub path: PathBuf,
    pub volume: f64,
    #[specta(type = specta_typescript::Number)]
    pub offset_ms: i64,
}

pub fn mix_audio(
    config: &FfmpegConfig,
    inputs: &[AudioMixInput],
    output: &Path,
    total_duration_ms: Option<i64>,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf> {
    if inputs.is_empty() {
        return Err(CoreError::Validation("mix_audio requires at least one input".into()));
    }
    for (i, inp) in inputs.iter().enumerate() {
        if !(0.0..=2.0).contains(&inp.volume) {
            return Err(CoreError::Validation(format!(
                "input[{i}] volume {:.1} out of range [0.0, 2.0]",
                inp.volume
            )));
        }
        if !inp.path.exists() {
            return Err(CoreError::Validation(format!(
                "input[{i}] file not found: {}",
                inp.path.display()
            )));
        }
    }

    let mut cmd = Command::new(config.ffmpeg_bin());
    cmd.arg("-y");

    for inp in inputs {
        cmd.arg("-i").arg(&inp.path);
    }

    let mut graph = FilterGraph::new();
    for (i, inp) in inputs.iter().enumerate() {
        let src = format!("{i}:a");
        let vol_out = format!("v{i}");
        graph.volume(&src, inp.volume, &vol_out);

        if inp.offset_ms > 0 {
            let delay_out = format!("d{i}");
            graph.adelay(&vol_out, inp.offset_ms, &delay_out);
        }
    }

    let mix_inputs: Vec<String> = inputs
        .iter()
        .enumerate()
        .map(|(i, inp)| {
            if inp.offset_ms > 0 {
                format!("d{i}")
            } else {
                format!("v{i}")
            }
        })
        .collect();
    let mix_refs: Vec<&str> = mix_inputs.iter().map(|s| s.as_str()).collect();

    if inputs.len() > 1 {
        graph.amix(&mix_refs, AmixDuration::Longest, "out");
        cmd.args(["-filter_complex", &graph.build()]);
        cmd.args(["-map", "[out]"]);
    } else {
        cmd.args(["-filter_complex", &graph.build()]);
        cmd.args(["-map", &format!("[{}]", mix_refs[0])]);
    }

    cmd.args(["-c:a", "aac", "-b:a", "128k"]);
    cmd.arg(output);

    let duration = total_duration_ms.unwrap_or(0);
    match on_progress {
        Some(cb) => run_ffmpeg_with_progress(&mut cmd, duration, cb)?,
        None => run_ffmpeg_simple(&mut cmd)?,
    }

    Ok(output.to_path_buf())
}

pub fn overlay_audio_on_video(
    config: &FfmpegConfig,
    video: &Path,
    audio_tracks: &[AudioMixInput],
    output: &Path,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf> {
    if audio_tracks.is_empty() {
        return Err(CoreError::Validation("overlay requires at least one audio track".into()));
    }
    if !video.exists() {
        return Err(CoreError::Validation(format!(
            "video file not found: {}",
            video.display()
        )));
    }
    for (i, t) in audio_tracks.iter().enumerate() {
        if !t.path.exists() {
            return Err(CoreError::Validation(format!(
                "audio track[{i}] file not found: {}",
                t.path.display()
            )));
        }
    }

    let mut cmd = Command::new(config.ffmpeg_bin());
    cmd.arg("-y");
    cmd.arg("-i").arg(video);
    for t in audio_tracks {
        cmd.arg("-i").arg(&t.path);
    }

    let mut graph = FilterGraph::new();
    // audio inputs start at index 1 (index 0 is the video)
    for (i, t) in audio_tracks.iter().enumerate() {
        let src = format!("{}:a", i + 1);
        let vol_out = format!("v{i}");
        graph.volume(&src, t.volume, &vol_out);

        if t.offset_ms > 0 {
            let delay_out = format!("d{i}");
            graph.adelay(&vol_out, t.offset_ms, &delay_out);
        }
    }

    let mix_inputs: Vec<String> = audio_tracks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            if t.offset_ms > 0 {
                format!("d{i}")
            } else {
                format!("v{i}")
            }
        })
        .collect();
    let mix_refs: Vec<&str> = mix_inputs.iter().map(|s| s.as_str()).collect();

    if audio_tracks.len() > 1 {
        graph.amix(&mix_refs, AmixDuration::Longest, "amixed");
    }

    let audio_map = if audio_tracks.len() > 1 {
        cmd.args(["-filter_complex", &graph.build()]);
        "[amixed]".to_string()
    } else {
        cmd.args(["-filter_complex", &graph.build()]);
        format!("[{}]", mix_refs[0])
    };

    cmd.args(["-map", "0:v"]);
    cmd.args(["-map", &audio_map]);
    cmd.args(["-c:v", "copy", "-c:a", "aac", "-b:a", "128k"]);
    cmd.arg(output);

    let probe = super::probe::probe_video(config, video)?;
    match on_progress {
        Some(cb) => run_ffmpeg_with_progress(&mut cmd, probe.duration_ms, cb)?,
        None => run_ffmpeg_simple(&mut cmd)?,
    }

    Ok(output.to_path_buf())
}

pub fn pad_audio_to_duration(
    config: &FfmpegConfig,
    input: &Path,
    target_duration_ms: i64,
    output: &Path,
) -> Result<PathBuf> {
    if !input.exists() {
        return Err(CoreError::Validation(format!(
            "audio file not found: {}",
            input.display()
        )));
    }
    if target_duration_ms <= 0 {
        return Err(CoreError::Validation("target_duration_ms must be positive".into()));
    }

    let mut graph = FilterGraph::new();
    graph.apad("0:a", target_duration_ms, "padded");
    graph.atrim("padded", target_duration_ms, "out");

    let mut cmd = Command::new(config.ffmpeg_bin());
    cmd.arg("-y")
        .arg("-i").arg(input)
        .args(["-filter_complex", &graph.build()])
        .args(["-map", "[out]"])
        .args(["-c:a", "aac", "-b:a", "128k"])
        .arg(output);

    run_ffmpeg_simple(&mut cmd)?;
    Ok(output.to_path_buf())
}

pub fn trim_audio(
    config: &FfmpegConfig,
    input: &Path,
    end_ms: i64,
    output: &Path,
) -> Result<PathBuf> {
    if !input.exists() {
        return Err(CoreError::Validation(format!(
            "audio file not found: {}",
            input.display()
        )));
    }
    if end_ms <= 0 {
        return Err(CoreError::Validation("end_ms must be positive".into()));
    }

    let end_secs = end_ms as f64 / 1000.0;
    let mut cmd = Command::new(config.ffmpeg_bin());
    cmd.arg("-y")
        .arg("-i").arg(input)
        .args(["-t", &format!("{end_secs}")])
        .args(["-c:a", "aac", "-b:a", "128k"])
        .arg(output);

    run_ffmpeg_simple(&mut cmd)?;
    Ok(output.to_path_buf())
}

pub fn probe_audio_duration(config: &FfmpegConfig, path: &Path) -> Result<i64> {
    if !path.exists() {
        return Err(CoreError::Validation(format!(
            "audio file not found: {}",
            path.display()
        )));
    }

    let output = Command::new(config.ffprobe_bin())
        .args(["-v", "quiet", "-print_format", "json", "-show_format"])
        .arg(path)
        .output()
        .map_err(|e| CoreError::Ffmpeg(format!("failed to run ffprobe: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::Ffmpeg(format!("ffprobe failed: {stderr}")));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| CoreError::Ffmpeg(format!("ffprobe JSON parse error: {e}")))?;

    let duration_str = json
        .pointer("/format/duration")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CoreError::Ffmpeg("ffprobe response missing format.duration".into()))?;

    let duration_secs: f64 = duration_str
        .parse()
        .map_err(|e| CoreError::Ffmpeg(format!("invalid duration value: {e}")))?;

    Ok((duration_secs * 1000.0) as i64)
}

fn run_ffmpeg_simple(cmd: &mut Command) -> Result<()> {
    let output = cmd
        .output()
        .map_err(|e| CoreError::Ffmpeg(format!("failed to execute ffmpeg: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::Ffmpeg(format!("ffmpeg exited with error: {stderr}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn mix_audio_empty_inputs_rejected() {
        let config = FfmpegConfig::from_env();
        let result = mix_audio(&config, &[], Path::new("/tmp/out.mp3"), None, None);
        assert!(result.is_err());
    }

    #[test]
    fn mix_audio_invalid_volume_rejected() {
        let config = FfmpegConfig::from_env();
        let inputs = vec![AudioMixInput {
            path: PathBuf::from("/nonexistent.mp3"),
            volume: 3.0,
            offset_ms: 0,
        }];
        let result = mix_audio(&config, &inputs, Path::new("/tmp/out.mp3"), None, None);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("volume"));
    }

    #[test]
    fn overlay_missing_video_rejected() {
        let config = FfmpegConfig::from_env();
        let tracks = vec![AudioMixInput {
            path: PathBuf::from("/nonexistent.mp3"),
            volume: 1.0,
            offset_ms: 0,
        }];
        let result = overlay_audio_on_video(
            &config,
            Path::new("/nonexistent_video.mp4"),
            &tracks,
            Path::new("/tmp/out.mp4"),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn overlay_empty_tracks_rejected() {
        let config = FfmpegConfig::from_env();
        let result = overlay_audio_on_video(
            &config,
            Path::new("/nonexistent_video.mp4"),
            &[],
            Path::new("/tmp/out.mp4"),
            None,
        );
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("at least one"));
    }

    #[test]
    fn pad_audio_invalid_duration_rejected() {
        let config = FfmpegConfig::from_env();
        let result = pad_audio_to_duration(
            &config,
            Path::new("/nonexistent.mp3"),
            -100,
            Path::new("/tmp/out.mp3"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn trim_audio_invalid_end_rejected() {
        let config = FfmpegConfig::from_env();
        let result = trim_audio(
            &config,
            Path::new("/nonexistent.mp3"),
            0,
            Path::new("/tmp/out.mp3"),
        );
        assert!(result.is_err());
    }

    #[test]
    fn probe_missing_file_rejected() {
        let config = FfmpegConfig::from_env();
        let result = probe_audio_duration(&config, Path::new("/nonexistent.mp3"));
        assert!(result.is_err());
    }
}
