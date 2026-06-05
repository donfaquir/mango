use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use specta::Type;
use ts_rs::TS;

use crate::error::{CoreError, Result};

use super::sidecar::FfmpegConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Type, TS)]
#[ts(export)]
pub struct ThumbnailStripResult {
    pub thumbnails: Vec<String>,
    #[specta(type = specta_typescript::Number)]
    pub interval_ms: i64,
    pub count: u32,
}

pub fn extract_thumbnail_strip(
    config: &FfmpegConfig,
    input: &Path,
    interval_ms: i64,
    thumb_width: u32,
    output_dir: &Path,
) -> Result<ThumbnailStripResult> {
    if interval_ms <= 0 {
        return Err(CoreError::Validation("interval_ms must be > 0".into()));
    }
    if thumb_width == 0 {
        return Err(CoreError::Validation("thumb_width must be > 0".into()));
    }
    if !input.exists() {
        return Err(CoreError::Validation(format!(
            "input file does not exist: {}",
            input.display()
        )));
    }

    if let Some(cached) = check_cache(output_dir, interval_ms) {
        return Ok(cached);
    }

    std::fs::create_dir_all(output_dir)?;

    let interval_s = interval_ms as f64 / 1000.0;
    let vf = format!("fps=1/{interval_s},scale={thumb_width}:-1");

    let output_pattern = output_dir.join("%04d.jpg");
    let mut cmd = Command::new(config.ffmpeg_bin());
    cmd.args(["-y", "-i"])
        .arg(input)
        .args(["-vf", &vf])
        .arg(&output_pattern);

    let output = cmd.output().map_err(|e| {
        CoreError::Ffmpeg(format!("failed to execute ffmpeg: {e}"))
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CoreError::Ffmpeg(format!(
            "ffmpeg thumbnail strip failed: {stderr}"
        )));
    }

    collect_results(output_dir, interval_ms)
}

fn check_cache(output_dir: &Path, interval_ms: i64) -> Option<ThumbnailStripResult> {
    if !output_dir.is_dir() {
        return None;
    }
    let entries = list_jpg_files(output_dir).ok()?;
    if entries.is_empty() {
        return None;
    }
    Some(ThumbnailStripResult {
        count: entries.len() as u32,
        thumbnails: entries,
        interval_ms,
    })
}

fn collect_results(output_dir: &Path, interval_ms: i64) -> Result<ThumbnailStripResult> {
    let thumbnails = list_jpg_files(output_dir)?;
    Ok(ThumbnailStripResult {
        count: thumbnails.len() as u32,
        thumbnails,
        interval_ms,
    })
}

fn list_jpg_files(dir: &Path) -> Result<Vec<String>> {
    let mut files: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("jpg"))
        })
        .filter_map(|e| e.path().to_str().map(|s| s.to_string()))
        .collect();
    files.sort();
    Ok(files)
}

pub fn thumbnail_strip_cache_key(input_path: &Path, interval_ms: i64, thumb_width: u32) -> String {
    use sha2::{Digest, Sha256};
    let path_str = input_path.to_string_lossy();
    let mut hasher = Sha256::new();
    hasher.update(path_str.as_bytes());
    let hash = hasher.finalize();
    let hash_hex: String = hash.iter().take(6).map(|b| format!("{b:02x}")).collect();
    format!("{hash_hex}_{interval_ms}_{thumb_width}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_deterministic() {
        let k1 = thumbnail_strip_cache_key(Path::new("/a/b.mp4"), 1000, 120);
        let k2 = thumbnail_strip_cache_key(Path::new("/a/b.mp4"), 1000, 120);
        assert_eq!(k1, k2);
    }

    #[test]
    fn cache_key_differs_by_interval() {
        let k1 = thumbnail_strip_cache_key(Path::new("/a/b.mp4"), 1000, 120);
        let k2 = thumbnail_strip_cache_key(Path::new("/a/b.mp4"), 2000, 120);
        assert_ne!(k1, k2);
    }

    #[test]
    fn cache_key_differs_by_path() {
        let k1 = thumbnail_strip_cache_key(Path::new("/a/b.mp4"), 1000, 120);
        let k2 = thumbnail_strip_cache_key(Path::new("/a/c.mp4"), 1000, 120);
        assert_ne!(k1, k2);
    }

    #[test]
    fn invalid_interval() {
        let config = FfmpegConfig { ffmpeg_path: None, ffprobe_path: None };
        let err = extract_thumbnail_strip(&config, Path::new("/x"), 0, 120, Path::new("/tmp"));
        assert!(err.unwrap_err().to_string().contains("interval_ms must be > 0"));
    }

    #[test]
    fn invalid_width() {
        let config = FfmpegConfig { ffmpeg_path: None, ffprobe_path: None };
        let err = extract_thumbnail_strip(&config, Path::new("/x"), 1000, 0, Path::new("/tmp"));
        assert!(err.unwrap_err().to_string().contains("thumb_width must be > 0"));
    }

    #[test]
    fn check_cache_empty_dir() {
        let dir = std::env::temp_dir().join("mango_test_cache_empty");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(check_cache(&dir, 1000).is_none());
    }
}
