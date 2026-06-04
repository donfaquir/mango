use std::path::Path;

use mango_core::ffmpeg::{check_ffmpeg, probe_video, FfmpegConfig};

/// Smoke test: verify check_ffmpeg detects a real FFmpeg installation.
/// Requires FFmpeg in PATH or MANGO_FFMPEG_PATH. Skip (ignore) in CI
/// where FFmpeg may not be installed.
#[test]
#[ignore]
fn check_ffmpeg_real() {
    let config = FfmpegConfig::from_env();
    let status = check_ffmpeg(&config);

    println!("FFmpeg available: {}", status.available);
    println!("FFmpeg version:   {:?}", status.version);
    println!("FFmpeg path:      {:?}", status.path);

    assert!(status.available, "FFmpeg should be available on this machine");
    assert!(status.version.is_some(), "version should be parsed");
}

/// Smoke test: generate a tiny video with FFmpeg, then probe it.
/// Requires FFmpeg in PATH.
#[test]
#[ignore]
fn probe_video_real() {
    let config = FfmpegConfig::from_env();

    // Generate a 2-second test video using FFmpeg's lavfi test source
    let tmp_dir = std::env::temp_dir().join("mango_ffmpeg_test");
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let test_video = tmp_dir.join("test_2s.mp4");

    let gen_status = std::process::Command::new(config.ffmpeg_bin())
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=2:size=320x240:rate=25",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=2",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-c:a",
            "aac",
            "-shortest",
        ])
        .arg(&test_video)
        .output()
        .expect("failed to generate test video");

    assert!(
        gen_status.status.success(),
        "ffmpeg should generate test video: {}",
        String::from_utf8_lossy(&gen_status.stderr)
    );
    assert!(test_video.exists(), "test video file should exist");

    // Probe the generated video
    let meta = probe_video(&config, &test_video).expect("probe should succeed");

    println!("Duration:    {} ms", meta.duration_ms);
    println!("Resolution:  {}x{}", meta.width, meta.height);
    println!("Video codec: {}", meta.video_codec);
    println!("Audio codec: {:?}", meta.audio_codec);
    println!("FPS:         {:.2}", meta.fps);
    println!("Bitrate:     {:?} kbps", meta.bitrate_kbps);
    println!("File size:   {} bytes", meta.file_size_bytes);

    assert!(meta.duration_ms >= 1800 && meta.duration_ms <= 2500, "duration ~2s");
    assert_eq!(meta.width, 320);
    assert_eq!(meta.height, 240);
    assert_eq!(meta.video_codec, "h264");
    assert!(meta.audio_codec.is_some(), "should have audio track");
    assert!(meta.fps > 24.0 && meta.fps < 26.0, "fps ~25");
    assert!(meta.file_size_bytes > 0);

    // Cleanup
    std::fs::remove_dir_all(&tmp_dir).ok();
}

/// Probe a non-existent file should return a clear error.
#[test]
#[ignore]
fn probe_video_missing_file() {
    let config = FfmpegConfig::from_env();
    let result = probe_video(&config, Path::new("/tmp/nonexistent_video.mp4"));
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    println!("Error: {err_msg}");
    assert!(err_msg.contains("does not exist"));
}
