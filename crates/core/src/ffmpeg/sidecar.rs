use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

use crate::error::{CoreError, Result};

#[derive(Debug, Clone)]
pub struct FfmpegConfig {
    pub ffmpeg_path: Option<PathBuf>,
    pub ffprobe_path: Option<PathBuf>,
}

impl FfmpegConfig {
    pub fn from_env() -> Self {
        let env_path = std::env::var("MANGO_FFMPEG_PATH").ok().map(PathBuf::from);
        FfmpegConfig {
            ffmpeg_path: env_path.clone(),
            ffprobe_path: env_path.map(|p| {
                let dir = p.parent().unwrap_or(Path::new("."));
                dir.join(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" })
            }),
        }
    }

    pub fn with_paths(ffmpeg: PathBuf, ffprobe: PathBuf) -> Self {
        FfmpegConfig {
            ffmpeg_path: Some(ffmpeg),
            ffprobe_path: Some(ffprobe),
        }
    }

    pub fn ffmpeg_bin(&self) -> &str {
        self.ffmpeg_path
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" })
    }

    pub fn ffprobe_bin(&self) -> &str {
        self.ffprobe_path
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct FfmpegStatus {
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

pub fn check_ffmpeg(config: &FfmpegConfig) -> FfmpegStatus {
    let bin = config.ffmpeg_bin();
    match Command::new(bin).arg("-version").output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let version = parse_version(&stdout);
            let resolved_path = resolve_binary_path(bin);
            FfmpegStatus {
                available: output.status.success(),
                version,
                path: resolved_path,
            }
        }
        Err(_) => FfmpegStatus {
            available: false,
            version: None,
            path: None,
        },
    }
}

fn resolve_binary_path(bin: &str) -> Option<String> {
    let cmd = if cfg!(windows) { "where" } else { "which" };
    Command::new(cmd)
        .arg(bin)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn parse_version(output: &str) -> Option<String> {
    let first_line = output.lines().next()?;
    // "ffmpeg version 7.1.1 Copyright ..." or "ffmpeg version N-12345-g..."
    let after_version = first_line.strip_prefix("ffmpeg version ")?;
    let version = after_version.split_whitespace().next()?;
    Some(version.to_string())
}

pub fn ensure_ffmpeg(config: &FfmpegConfig) -> Result<()> {
    let status = check_ffmpeg(config);
    if !status.available {
        return Err(CoreError::Ffmpeg(
            "FFmpeg is not available. Please install FFmpeg or check MANGO_FFMPEG_PATH."
                .to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_standard() {
        let output = "ffmpeg version 7.1.1 Copyright (c) 2000-2025 the FFmpeg developers";
        assert_eq!(parse_version(output), Some("7.1.1".to_string()));
    }

    #[test]
    fn parse_version_n_series() {
        let output = "ffmpeg version N-12345-gabcdef Copyright (c) 2000-2025";
        assert_eq!(
            parse_version(output),
            Some("N-12345-gabcdef".to_string())
        );
    }

    #[test]
    fn parse_version_with_extras() {
        let output =
            "ffmpeg version 6.1.2-1ubuntu1 Copyright (c) 2000-2024 the FFmpeg developers";
        assert_eq!(parse_version(output), Some("6.1.2-1ubuntu1".to_string()));
    }

    #[test]
    fn parse_version_empty() {
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn parse_version_no_prefix() {
        assert_eq!(parse_version("something else entirely"), None);
    }

    #[test]
    fn config_default_bins() {
        let config = FfmpegConfig {
            ffmpeg_path: None,
            ffprobe_path: None,
        };
        assert_eq!(config.ffmpeg_bin(), "ffmpeg");
        assert_eq!(config.ffprobe_bin(), "ffprobe");
    }

    #[test]
    fn config_with_explicit_paths() {
        let config = FfmpegConfig::with_paths(
            PathBuf::from("/usr/local/bin/ffmpeg"),
            PathBuf::from("/usr/local/bin/ffprobe"),
        );
        assert_eq!(config.ffmpeg_bin(), "/usr/local/bin/ffmpeg");
        assert_eq!(config.ffprobe_bin(), "/usr/local/bin/ffprobe");
    }
}
