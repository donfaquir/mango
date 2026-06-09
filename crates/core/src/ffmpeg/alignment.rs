//! Audio-video duration alignment strategy.

use std::path::{Path, PathBuf};

use super::audio;
use super::sidecar::FfmpegConfig;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq)]
pub enum AlignmentStrategy {
    Exact,
    PadSilence { gap_ms: i64 },
    TrimAudio { excess_ms: i64 },
    RegenerateNeeded { excess_ms: i64, suggested_speed: f64 },
}

const DEFAULT_TOLERANCE_MS: i64 = 500;
const DEFAULT_TRIM_THRESHOLD_MS: i64 = 3000;

pub fn calculate_alignment(
    video_duration_ms: i64,
    audio_duration_ms: i64,
    tolerance_ms: i64,
    trim_threshold_ms: i64,
) -> AlignmentStrategy {
    let diff = audio_duration_ms - video_duration_ms;

    if diff.abs() <= tolerance_ms {
        return AlignmentStrategy::Exact;
    }
    if diff < 0 {
        return AlignmentStrategy::PadSilence { gap_ms: diff.abs() };
    }
    if diff <= trim_threshold_ms {
        return AlignmentStrategy::TrimAudio { excess_ms: diff };
    }
    AlignmentStrategy::RegenerateNeeded {
        excess_ms: diff,
        suggested_speed: audio_duration_ms as f64 / video_duration_ms as f64,
    }
}

pub fn calculate_alignment_default(
    video_duration_ms: i64,
    audio_duration_ms: i64,
) -> AlignmentStrategy {
    calculate_alignment(
        video_duration_ms,
        audio_duration_ms,
        DEFAULT_TOLERANCE_MS,
        DEFAULT_TRIM_THRESHOLD_MS,
    )
}

pub fn apply_alignment(
    config: &FfmpegConfig,
    audio_path: &Path,
    video_duration_ms: i64,
    strategy: &AlignmentStrategy,
    output_dir: &Path,
) -> Result<PathBuf> {
    match strategy {
        AlignmentStrategy::Exact => Ok(audio_path.to_path_buf()),
        AlignmentStrategy::PadSilence { .. } => {
            let output = output_dir.join("aligned.mp3");
            audio::pad_audio_to_duration(config, audio_path, video_duration_ms, &output)
        }
        AlignmentStrategy::TrimAudio { .. } => {
            let output = output_dir.join("aligned.mp3");
            audio::trim_audio(config, audio_path, video_duration_ms, &output)
        }
        AlignmentStrategy::RegenerateNeeded { .. } => {
            Ok(audio_path.to_path_buf())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_within_tolerance() {
        let s = calculate_alignment(5000, 5300, 500, 3000);
        assert_eq!(s, AlignmentStrategy::Exact);
    }

    #[test]
    fn exact_negative_within_tolerance() {
        let s = calculate_alignment(5000, 4700, 500, 3000);
        assert_eq!(s, AlignmentStrategy::Exact);
    }

    #[test]
    fn pad_silence_when_audio_shorter() {
        let s = calculate_alignment(5000, 3000, 500, 3000);
        assert_eq!(s, AlignmentStrategy::PadSilence { gap_ms: 2000 });
    }

    #[test]
    fn trim_audio_when_slightly_longer() {
        let s = calculate_alignment(5000, 6500, 500, 3000);
        assert_eq!(s, AlignmentStrategy::TrimAudio { excess_ms: 1500 });
    }

    #[test]
    fn regenerate_when_much_longer() {
        let s = calculate_alignment(5000, 10000, 500, 3000);
        assert!(matches!(s, AlignmentStrategy::RegenerateNeeded { excess_ms: 5000, .. }));
        if let AlignmentStrategy::RegenerateNeeded { suggested_speed, .. } = s {
            assert!((suggested_speed - 2.0).abs() < 0.01);
        }
    }

    #[test]
    fn default_tolerance_works() {
        let s = calculate_alignment_default(5000, 5400);
        assert_eq!(s, AlignmentStrategy::Exact);
    }
}
