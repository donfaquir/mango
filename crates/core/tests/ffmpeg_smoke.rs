use std::path::Path;

use mango_core::ffmpeg::{
    check_ffmpeg, concat_videos, extract_thumbnail, extract_thumbnail_strip, probe_video,
    split_video, trim_video, FfmpegConfig, FfmpegProgress, TrimMode,
};

// ---------------------------------------------------------------------------
// Helper: generate a test video with lavfi
// ---------------------------------------------------------------------------

fn test_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mango_ffmpeg_test_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn generate_test_video(config: &FfmpegConfig, path: &Path, duration_secs: u32, width: u32, height: u32) {
    let video_src = format!("testsrc=duration={duration_secs}:size={width}x{height}:rate=25");
    let audio_src = format!("sine=frequency=440:duration={duration_secs}");
    let status = std::process::Command::new(config.ffmpeg_bin())
        .args(["-y", "-f", "lavfi", "-i", &video_src, "-f", "lavfi", "-i", &audio_src])
        .args(["-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac", "-shortest"])
        .arg(path)
        .output()
        .expect("failed to run ffmpeg");
    assert!(status.status.success(), "generate video failed: {}", String::from_utf8_lossy(&status.stderr));
}

fn generate_video_only(config: &FfmpegConfig, path: &Path, duration_secs: u32) {
    let video_src = format!("testsrc=duration={duration_secs}:size=320x240:rate=25");
    let status = std::process::Command::new(config.ffmpeg_bin())
        .args(["-y", "-f", "lavfi", "-i", &video_src])
        .args(["-c:v", "libx264", "-preset", "ultrafast", "-an"])
        .arg(path)
        .output()
        .expect("failed to run ffmpeg");
    assert!(status.status.success(), "generate video-only failed: {}", String::from_utf8_lossy(&status.stderr));
}

// ---------------------------------------------------------------------------
// spec-32 tests
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn check_ffmpeg_real() {
    let config = FfmpegConfig::from_env();
    let status = check_ffmpeg(&config);
    println!("FFmpeg available: {}", status.available);
    println!("FFmpeg version:   {:?}", status.version);
    println!("FFmpeg path:      {:?}", status.path);
    assert!(status.available);
    assert!(status.version.is_some());
}

#[test]
#[ignore]
fn probe_video_real() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("probe");
    let video = tmp.join("probe_test.mp4");
    generate_test_video(&config, &video, 2, 320, 240);

    let meta = probe_video(&config, &video).unwrap();
    println!("{meta:?}");
    assert!(meta.duration_ms >= 1800 && meta.duration_ms <= 2500);
    assert_eq!(meta.width, 320);
    assert_eq!(meta.height, 240);
    assert_eq!(meta.video_codec, "h264");
    assert!(meta.audio_codec.is_some());
    assert!(meta.fps > 24.0 && meta.fps < 26.0);

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
#[ignore]
fn probe_video_missing_file() {
    let config = FfmpegConfig::from_env();
    let err = probe_video(&config, Path::new("/tmp/nonexistent_video.mp4"));
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("does not exist"));
}

// ---------------------------------------------------------------------------
// spec-33 tests: trim
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn trim_copy_real() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("trim_copy");
    let src = tmp.join("trim_src.mp4");
    generate_test_video(&config, &src, 5, 320, 240);

    let out = tmp.join("trim_copy_out.mp4");
    trim_video(&config, &src, 1000, 4000, &out, &TrimMode::Copy, None).unwrap();

    assert!(out.exists());
    let meta = probe_video(&config, &out).unwrap();
    println!("Trimmed (copy) duration: {} ms", meta.duration_ms);
    assert!(meta.duration_ms >= 2500 && meta.duration_ms <= 3500, "duration ~3s");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
#[ignore]
fn trim_reencode_real() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("trim_reencode");
    let src = tmp.join("trim_reencode_src.mp4");
    generate_test_video(&config, &src, 5, 320, 240);

    let out = tmp.join("trim_reencode_out.mp4");
    trim_video(&config, &src, 1000, 4000, &out, &TrimMode::Reencode, None).unwrap();

    assert!(out.exists());
    let meta = probe_video(&config, &out).unwrap();
    println!("Trimmed (reencode) duration: {} ms", meta.duration_ms);
    assert!(meta.duration_ms >= 2800 && meta.duration_ms <= 3200, "duration ~3s");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
#[ignore]
fn trim_reencode_no_audio() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("trim_noaudio");
    let src = tmp.join("trim_noaudio_src.mp4");
    generate_video_only(&config, &src, 3);

    let out = tmp.join("trim_noaudio_out.mp4");
    trim_video(&config, &src, 500, 2500, &out, &TrimMode::Reencode, None).unwrap();

    assert!(out.exists());
    let meta = probe_video(&config, &out).unwrap();
    println!("Trimmed (no audio) duration: {} ms, audio: {:?}", meta.duration_ms, meta.audio_codec);
    assert!(meta.audio_codec.is_none());

    std::fs::remove_dir_all(&tmp).ok();
}

// ---------------------------------------------------------------------------
// spec-33 tests: split
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn split_two_points_real() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("split");
    let src = tmp.join("split_src.mp4");
    generate_test_video(&config, &src, 6, 320, 240);

    let out_dir = tmp.join("split_out");
    std::fs::create_dir_all(&out_dir).unwrap();

    let parts = split_video(&config, &src, &[2000, 4000], &out_dir, &TrimMode::Copy, None).unwrap();
    println!("Split produced {} parts", parts.len());
    assert_eq!(parts.len(), 3);
    for p in &parts {
        assert!(p.exists(), "part should exist: {}", p.display());
        let meta = probe_video(&config, p).unwrap();
        println!("  {}: {} ms", p.file_name().unwrap().to_string_lossy(), meta.duration_ms);
    }

    std::fs::remove_dir_all(&tmp).ok();
}

// ---------------------------------------------------------------------------
// spec-33 tests: concat
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn concat_same_params_real() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("concat_same");

    let a = tmp.join("concat_a.mp4");
    let b = tmp.join("concat_b.mp4");
    generate_test_video(&config, &a, 2, 320, 240);
    generate_test_video(&config, &b, 3, 320, 240);

    let out = tmp.join("concat_out.mp4");
    concat_videos(&config, &[a, b], &out, None).unwrap();

    assert!(out.exists());
    let meta = probe_video(&config, &out).unwrap();
    println!("Concat duration: {} ms", meta.duration_ms);
    assert!(meta.duration_ms >= 4500 && meta.duration_ms <= 5500, "duration ~5s");

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
#[ignore]
fn concat_diff_params_error() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("concat_diff");

    let a = tmp.join("concat_diff_a.mp4");
    let b = tmp.join("concat_diff_b.mp4");
    generate_test_video(&config, &a, 2, 320, 240);
    generate_test_video(&config, &b, 2, 640, 480);

    let out = tmp.join("concat_diff_out.mp4");
    let err = concat_videos(&config, &[a, b], &out, None);
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    println!("Expected error: {msg}");
    assert!(msg.contains("resolution"));

    std::fs::remove_dir_all(&tmp).ok();
}

// ---------------------------------------------------------------------------
// spec-33 tests: thumbnail
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn thumbnail_real() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("thumbnail");
    let src = tmp.join("thumb_src.mp4");
    generate_test_video(&config, &src, 3, 320, 240);

    let out = tmp.join("thumb_1s.jpg");
    extract_thumbnail(&config, &src, 1000, &out).unwrap();

    assert!(out.exists());
    let size = std::fs::metadata(&out).unwrap().len();
    println!("Thumbnail size: {} bytes", size);
    assert!(size > 100, "thumbnail should not be empty");

    std::fs::remove_dir_all(&tmp).ok();
}

// ---------------------------------------------------------------------------
// spec-34 tests: progress
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn trim_with_progress() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("trim_progress");
    let src = tmp.join("progress_src.mp4");
    generate_test_video(&config, &src, 5, 320, 240);

    let mut ticks: Vec<f64> = Vec::new();
    let mut cb = |p: FfmpegProgress| {
        println!("  progress: {:.1}% speed={:?}", p.progress_pct, p.speed);
        ticks.push(p.progress_pct);
    };

    let out = tmp.join("progress_out.mp4");
    trim_video(&config, &src, 0, 5000, &out, &TrimMode::Reencode, Some(&mut cb)).unwrap();

    assert!(out.exists());
    println!("Total progress ticks: {}", ticks.len());
    assert!(!ticks.is_empty(), "should receive at least one progress tick");

    for i in 1..ticks.len() {
        assert!(ticks[i] >= ticks[i - 1], "progress should be monotonically increasing");
    }

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
#[ignore]
fn progress_none_still_works() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("progress_none");
    let src = tmp.join("none_src.mp4");
    generate_test_video(&config, &src, 2, 320, 240);

    let out = tmp.join("none_out.mp4");
    trim_video(&config, &src, 0, 2000, &out, &TrimMode::Copy, None).unwrap();
    assert!(out.exists());

    std::fs::remove_dir_all(&tmp).ok();
}

// ---------------------------------------------------------------------------
// spec-35 tests: thumbnail strip
// ---------------------------------------------------------------------------

#[test]
#[ignore]
fn thumbnail_strip_real() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("thumb_strip");
    let src = tmp.join("strip_src.mp4");
    generate_test_video(&config, &src, 5, 320, 240);

    let out_dir = tmp.join("strip_out");
    let result = extract_thumbnail_strip(&config, &src, 1000, 120, &out_dir).unwrap();

    println!("Thumbnail strip: {} images", result.count);
    assert!(result.count >= 4 && result.count <= 6, "should produce ~5 thumbnails for 5s video");
    for path in &result.thumbnails {
        assert!(std::path::Path::new(path).exists(), "thumbnail should exist: {path}");
    }

    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
#[ignore]
fn thumbnail_strip_cache_hit() {
    let config = FfmpegConfig::from_env();
    let tmp = test_dir("thumb_cache");
    let src = tmp.join("cache_src.mp4");
    generate_test_video(&config, &src, 3, 320, 240);

    let out_dir = tmp.join("cache_out");
    let r1 = extract_thumbnail_strip(&config, &src, 1000, 120, &out_dir).unwrap();
    let r2 = extract_thumbnail_strip(&config, &src, 1000, 120, &out_dir).unwrap();

    assert_eq!(r1.count, r2.count, "cache should return same count");

    std::fs::remove_dir_all(&tmp).ok();
}
