use std::io::BufRead;
use std::process::{Command, Stdio};

use crate::error::{CoreError, Result};

#[derive(Debug, Clone)]
pub struct FfmpegProgress {
    pub progress_pct: f64,
    pub current_time_ms: i64,
    pub total_duration_ms: i64,
    pub speed: Option<f64>,
}

pub struct FfmpegProgressParser {
    total_duration_ms: i64,
}

impl FfmpegProgressParser {
    pub fn new(total_duration_ms: i64) -> Self {
        Self { total_duration_ms }
    }

    pub fn parse_line(&self, line: &str) -> Option<FfmpegProgress> {
        let current_time_ms = parse_time_field(line)?;
        let progress_pct = if self.total_duration_ms > 0 {
            (current_time_ms as f64 / self.total_duration_ms as f64 * 100.0).min(100.0)
        } else {
            0.0
        };
        let speed = parse_speed_field(line);

        Some(FfmpegProgress {
            progress_pct,
            current_time_ms,
            total_duration_ms: self.total_duration_ms,
            speed,
        })
    }
}

fn parse_time_field(line: &str) -> Option<i64> {
    let time_start = line.find("time=")?;
    let after = &line[time_start + 5..];
    let time_str = after.split_whitespace().next()?;

    // HH:MM:SS.ff or HH:MM:SS.fff
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 3 {
        return None;
    }

    let hours: i64 = parts[0].parse().ok()?;
    let minutes: i64 = parts[1].parse().ok()?;

    let sec_parts: Vec<&str> = parts[2].split('.').collect();
    let seconds: i64 = sec_parts[0].parse().ok()?;
    let centis: i64 = sec_parts.get(1).and_then(|s| {
        match s.len() {
            2 => s.parse::<i64>().ok().map(|v| v * 10),
            3 => s.parse::<i64>().ok(),
            1 => s.parse::<i64>().ok().map(|v| v * 100),
            _ => s.get(..3).and_then(|s3| s3.parse().ok()),
        }
    }).unwrap_or(0);

    Some(hours * 3_600_000 + minutes * 60_000 + seconds * 1000 + centis)
}

fn parse_speed_field(line: &str) -> Option<f64> {
    let speed_start = line.find("speed=")?;
    let after = &line[speed_start + 6..];
    let speed_str = after.split_whitespace().next()?;
    let speed_str = speed_str.trim_end_matches('x');
    speed_str.parse().ok()
}

pub fn run_ffmpeg_with_progress(
    cmd: &mut Command,
    total_duration_ms: i64,
    mut on_progress: impl FnMut(FfmpegProgress),
) -> Result<()> {
    cmd.stdout(Stdio::null()).stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        CoreError::Ffmpeg(format!("failed to spawn ffmpeg: {e}"))
    })?;

    let stderr = child.stderr.take().ok_or_else(|| {
        CoreError::Ffmpeg("failed to capture ffmpeg stderr".into())
    })?;

    let parser = FfmpegProgressParser::new(total_duration_ms);
    let reader = std::io::BufReader::new(stderr);

    let mut last_stderr = String::new();
    for line in reader.lines() {
        match line {
            Ok(l) => {
                if let Some(progress) = parser.parse_line(&l) {
                    on_progress(progress);
                }
                last_stderr = l;
            }
            Err(_) => break,
        }
    }

    let status = child.wait().map_err(|e| {
        CoreError::Ffmpeg(format!("failed to wait for ffmpeg: {e}"))
    })?;

    if !status.success() {
        return Err(CoreError::Ffmpeg(format!("ffmpeg exited with error: {last_stderr}")));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_line() {
        let parser = FfmpegProgressParser::new(10_000);
        let line = "frame=  120 fps= 30 q=28.0 size=    1024kB time=00:00:04.50 bitrate= 2097.2kbits/s speed=2.00x";
        let p = parser.parse_line(line).unwrap();
        assert!((p.progress_pct - 45.0).abs() < 0.1);
        assert_eq!(p.current_time_ms, 4500);
        assert_eq!(p.total_duration_ms, 10_000);
        assert!((p.speed.unwrap() - 2.0).abs() < 0.01);
    }

    #[test]
    fn parse_with_three_digit_millis() {
        let parser = FfmpegProgressParser::new(5000);
        let line = "time=00:00:02.500 speed=1.50x";
        let p = parser.parse_line(line).unwrap();
        assert_eq!(p.current_time_ms, 2500);
        assert!((p.progress_pct - 50.0).abs() < 0.1);
    }

    #[test]
    fn parse_hour_minute() {
        let parser = FfmpegProgressParser::new(3_700_000);
        let line = "time=01:01:39.50 speed=1.00x";
        let p = parser.parse_line(line).unwrap();
        assert_eq!(p.current_time_ms, 3_699_500);
    }

    #[test]
    fn parse_no_speed() {
        let parser = FfmpegProgressParser::new(10_000);
        let line = "frame=  60 time=00:00:02.00 bitrate=1000kbits/s";
        let p = parser.parse_line(line).unwrap();
        assert_eq!(p.current_time_ms, 2000);
        assert!(p.speed.is_none());
    }

    #[test]
    fn parse_no_time() {
        let parser = FfmpegProgressParser::new(10_000);
        let line = "Input #0, mov,mp4,m4a from 'test.mp4':";
        assert!(parser.parse_line(line).is_none());
    }

    #[test]
    fn parse_zero_duration() {
        let parser = FfmpegProgressParser::new(0);
        let line = "time=00:00:01.00 speed=1.00x";
        let p = parser.parse_line(line).unwrap();
        assert_eq!(p.progress_pct, 0.0);
        assert_eq!(p.current_time_ms, 1000);
    }

    #[test]
    fn parse_time_zero() {
        let parser = FfmpegProgressParser::new(10_000);
        let line = "time=00:00:00.00 speed=0.00x";
        let p = parser.parse_line(line).unwrap();
        assert_eq!(p.current_time_ms, 0);
        assert_eq!(p.progress_pct, 0.0);
    }

    #[test]
    fn progress_pct_capped_at_100() {
        let parser = FfmpegProgressParser::new(5000);
        let line = "time=00:00:06.00 speed=1.00x";
        let p = parser.parse_line(line).unwrap();
        assert!((p.progress_pct - 100.0).abs() < 0.01);
    }
}
