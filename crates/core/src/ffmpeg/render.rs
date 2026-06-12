use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::{CoreError, Result};

use super::audio::AudioMixInput;
use super::filter_graph::FilterGraph;
use super::progress::{run_ffmpeg_with_progress, FfmpegProgress};
use super::sidecar::FfmpegConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct RenderConfig {
    pub video_codec: String,
    pub preset: String,
    pub crf: u32,
    pub audio_bitrate: String,
    pub container: String,
    pub output_width: Option<i32>,
    pub output_height: Option<i32>,
    pub output_fps: Option<f64>,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            video_codec: "libx264".into(),
            preset: "fast".into(),
            crf: 18,
            audio_bitrate: "128k".into(),
            container: "mp4".into(),
            output_width: None,
            output_height: None,
            output_fps: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KenBurnsParams {
    pub zoom_expr: String,
    pub x_expr: String,
    pub y_expr: String,
}

#[derive(Debug, Clone)]
pub enum ColorEffectParams {
    Colorbalance {
        rs: f64,
        gs: f64,
        bs: f64,
        rm: f64,
        gm: f64,
        bm: f64,
        rh: f64,
        gh: f64,
        bh: f64,
    },
    Eq {
        brightness: f64,
        contrast: f64,
        saturation: f64,
    },
}

#[derive(Debug)]
pub struct ResolvedTimelineClip {
    pub source_path: PathBuf,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub in_point_ms: i64,
    pub out_point_ms: i64,
    pub ken_burns: Option<KenBurnsParams>,
    pub color_effect: Option<ColorEffectParams>,
}

#[derive(Debug)]
pub struct ResolvedTransition {
    pub transition_type: String,
    pub duration_ms: i64,
    pub position_ms: i64,
}

#[derive(Debug)]
pub struct ResolvedTextOverlay {
    pub text: String,
    pub fontfile: Option<String>,
    pub fontsize: u32,
    pub fontcolor: String,
    pub x: String,
    pub y: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug)]
pub struct ResolvedStickerOverlay {
    pub image_path: PathBuf,
    pub x: String,
    pub y: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

#[derive(Debug)]
pub struct ResolvedTimeline {
    pub clips: Vec<ResolvedTimelineClip>,
    pub transitions: Vec<ResolvedTransition>,
    pub text_overlays: Vec<ResolvedTextOverlay>,
    pub sticker_overlays: Vec<ResolvedStickerOverlay>,
    pub audio_tracks: Vec<AudioMixInput>,
    pub total_duration_ms: i64,
}

pub fn build_filter_graph(
    timeline: &ResolvedTimeline,
    config: &RenderConfig,
) -> Result<(FilterGraph, String)> {
    if timeline.clips.is_empty() {
        return Err(CoreError::Validation("no clips in timeline".into()));
    }

    let width = config
        .output_width
        .unwrap_or_else(|| guess_width(&timeline.clips[0]));
    let height = config
        .output_height
        .unwrap_or_else(|| guess_height(&timeline.clips[0]));
    let fps = config.output_fps.unwrap_or(30.0);

    let mut graph = FilterGraph::new();
    let clip_count = timeline.clips.len();

    // Phase 1: normalize each clip
    let mut normalized: Vec<String> = Vec::with_capacity(clip_count);
    for i in 0..clip_count {
        let label = format!("n{i}");
        graph.normalize(&format!("{i}:v"), width, height, fps, "yuv420p", &label);
        normalized.push(label);
    }

    // Phase 2: apply per-clip effects (ken burns, color)
    let mut effected: Vec<String> = Vec::with_capacity(clip_count);
    for (i, clip) in timeline.clips.iter().enumerate() {
        let mut current = normalized[i].clone();

        if let Some(kb) = &clip.ken_burns {
            let out = format!("kb{i}");
            let frames = ((clip.out_point_ms - clip.in_point_ms) as f64 / 1000.0 * fps) as u32;
            graph.zoompan(
                &current,
                &kb.zoom_expr,
                &kb.x_expr,
                &kb.y_expr,
                frames,
                width as u32,
                height as u32,
                fps as u32,
                &out,
            );
            current = out;
        }

        if let Some(ce) = &clip.color_effect {
            let out = format!("ce{i}");
            match ce {
                ColorEffectParams::Colorbalance {
                    rs,
                    gs,
                    bs,
                    rm,
                    gm,
                    bm,
                    rh,
                    gh,
                    bh,
                } => {
                    graph.colorbalance(&current, *rs, *gs, *bs, *rm, *gm, *bm, *rh, *gh, *bh, &out);
                }
                ColorEffectParams::Eq {
                    brightness,
                    contrast,
                    saturation,
                } => {
                    graph.eq(&current, *brightness, *contrast, *saturation, &out);
                }
            }
            current = out;
        }

        effected.push(current);
    }

    // Phase 3: join clips with xfade transitions or concat
    let mut video_out = if timeline.transitions.is_empty() {
        if effected.len() == 1 {
            effected[0].clone()
        } else {
            let inputs: Vec<&str> = effected.iter().map(|s| s.as_str()).collect();
            graph.video_concat(&inputs, inputs.len(), false, "vconcat", None);
            "vconcat".to_string()
        }
    } else {
        let mut current = effected[0].clone();
        let mut offset_secs = 0.0;

        for (i, _clip) in timeline.clips.iter().enumerate().skip(1) {
            let prev_dur = (timeline.clips[i - 1].out_point_ms
                - timeline.clips[i - 1].in_point_ms) as f64
                / 1000.0;

            let transition = timeline.transitions.iter().find(|t| {
                let t_pos = t.position_ms as f64 / 1000.0;
                (t_pos - (offset_secs + prev_dur)).abs() < 0.1
            });

            offset_secs += prev_dur;

            if let Some(tr) = transition {
                let dur_secs = tr.duration_ms as f64 / 1000.0;
                let xf_offset = offset_secs - dur_secs;
                let out = format!("xf{i}");
                graph.xfade(
                    &current,
                    &effected[i],
                    &tr.transition_type,
                    dur_secs,
                    xf_offset,
                    &out,
                );
                offset_secs -= dur_secs;
                current = out;
            } else {
                let inputs = [current.as_str(), effected[i].as_str()];
                let out = format!("cat{i}");
                graph.video_concat(&inputs, 2, false, &out, None);
                current = out;
            }
        }
        current
    };

    // Phase 4: text overlays
    for (i, text) in timeline.text_overlays.iter().enumerate() {
        let out = format!("txt{i}");
        let start_s = text.start_ms as f64 / 1000.0;
        let end_s = text.end_ms as f64 / 1000.0;
        let enable = format!("between(t,{start_s},{end_s})");
        graph.drawtext(
            &video_out,
            &text.text,
            text.fontfile.as_deref(),
            text.fontsize,
            &text.fontcolor,
            &text.x,
            &text.y,
            Some(&enable),
            &out,
        );
        video_out = out;
    }

    // Phase 5: sticker overlays (require separate input streams, handled at command build time)
    // Sticker overlays are referenced by label but their input indices must be offset
    // by the number of video clip inputs. For now, track the labels.
    for (i, _sticker) in timeline.sticker_overlays.iter().enumerate() {
        let sticker_input = format!("stk_in{i}");
        let out = format!("stk{i}");
        let start_s = timeline.sticker_overlays[i].start_ms as f64 / 1000.0;
        let end_s = timeline.sticker_overlays[i].end_ms as f64 / 1000.0;
        let enable = format!("between(t,{start_s},{end_s})");
        graph.overlay(
            &video_out,
            &sticker_input,
            &timeline.sticker_overlays[i].x,
            &timeline.sticker_overlays[i].y,
            Some(&enable),
            &out,
        );
        video_out = out;
    }

    Ok((graph, video_out))
}

fn guess_width(_clip: &ResolvedTimelineClip) -> i32 {
    1920
}

fn guess_height(_clip: &ResolvedTimelineClip) -> i32 {
    1080
}

pub fn render_timeline(
    config: &RenderConfig,
    ffmpeg_config: &FfmpegConfig,
    timeline: &ResolvedTimeline,
    output_path: &Path,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf> {
    if timeline.clips.is_empty() {
        return Err(CoreError::Validation("no clips to render".into()));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let (graph, video_out_label) = build_filter_graph(timeline, config)?;
    let filter_complex = graph.build();

    let mut cmd = Command::new(ffmpeg_config.ffmpeg_bin());
    cmd.arg("-y");

    for clip in &timeline.clips {
        cmd.arg("-i").arg(&clip.source_path);
    }
    for sticker in &timeline.sticker_overlays {
        cmd.arg("-i").arg(&sticker.image_path);
    }
    for audio in &timeline.audio_tracks {
        cmd.arg("-i").arg(&audio.path);
    }

    cmd.args(["-filter_complex", &filter_complex]);
    cmd.args(["-map", &format!("[{video_out_label}]")]);

    if !timeline.audio_tracks.is_empty() {
        cmd.args(["-c:a", "aac", "-b:a", &config.audio_bitrate]);
    } else {
        cmd.arg("-an");
    }

    cmd.args(["-c:v", &config.video_codec]);
    cmd.args(["-preset", &config.preset]);
    cmd.args(["-crf", &config.crf.to_string()]);
    cmd.arg(output_path);

    match on_progress {
        Some(cb) => run_ffmpeg_with_progress(&mut cmd, timeline.total_duration_ms, cb)?,
        None => {
            let output = cmd.output().map_err(|e| {
                CoreError::Ffmpeg(format!("failed to execute ffmpeg: {e}"))
            })?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CoreError::Ffmpeg(format!("ffmpeg render failed: {stderr}")));
            }
        }
    }

    Ok(output_path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_config_default() {
        let c = RenderConfig::default();
        assert_eq!(c.video_codec, "libx264");
        assert_eq!(c.preset, "fast");
        assert_eq!(c.crf, 18);
        assert_eq!(c.audio_bitrate, "128k");
        assert_eq!(c.container, "mp4");
        assert!(c.output_width.is_none());
        assert!(c.output_height.is_none());
        assert!(c.output_fps.is_none());
    }

    fn make_clip(path: &str, duration_ms: i64) -> ResolvedTimelineClip {
        ResolvedTimelineClip {
            source_path: PathBuf::from(path),
            position_ms: 0,
            duration_ms,
            in_point_ms: 0,
            out_point_ms: duration_ms,
            ken_burns: None,
            color_effect: None,
        }
    }

    fn empty_timeline(clips: Vec<ResolvedTimelineClip>) -> ResolvedTimeline {
        let total = clips.iter().map(|c| c.duration_ms).sum();
        ResolvedTimeline {
            clips,
            transitions: vec![],
            text_overlays: vec![],
            sticker_overlays: vec![],
            audio_tracks: vec![],
            total_duration_ms: total,
        }
    }

    #[test]
    fn build_graph_empty_timeline_errors() {
        let timeline = empty_timeline(vec![]);
        let config = RenderConfig::default();
        assert!(build_filter_graph(&timeline, &config).is_err());
    }

    #[test]
    fn build_graph_single_clip() {
        let timeline = empty_timeline(vec![make_clip("/v0.mp4", 5000)]);
        let config = RenderConfig {
            output_width: Some(1280),
            output_height: Some(720),
            output_fps: Some(30.0),
            ..Default::default()
        };

        let (graph, out_label) = build_filter_graph(&timeline, &config).unwrap();
        let fc = graph.build();
        assert_eq!(fc, "[0:v]scale=1280:720,fps=30,format=yuv420p[n0]");
        assert_eq!(out_label, "n0");
    }

    #[test]
    fn build_graph_two_clips_no_transition() {
        let timeline = empty_timeline(vec![
            make_clip("/v0.mp4", 5000),
            make_clip("/v1.mp4", 3000),
        ]);
        let config = RenderConfig {
            output_width: Some(1920),
            output_height: Some(1080),
            ..Default::default()
        };

        let (graph, out_label) = build_filter_graph(&timeline, &config).unwrap();
        let fc = graph.build();
        assert!(fc.contains("[0:v]scale=1920:1080"), "got: {fc}");
        assert!(fc.contains("[1:v]scale=1920:1080"), "got: {fc}");
        assert!(fc.contains("concat=n=2:v=1:a=0"), "got: {fc}");
        assert_eq!(out_label, "vconcat");
    }

    #[test]
    fn build_graph_with_xfade() {
        let mut timeline = empty_timeline(vec![
            make_clip("/v0.mp4", 5000),
            make_clip("/v1.mp4", 5000),
        ]);
        timeline.transitions.push(ResolvedTransition {
            transition_type: "dissolve".into(),
            duration_ms: 500,
            position_ms: 5000,
        });
        let config = RenderConfig {
            output_width: Some(1920),
            output_height: Some(1080),
            ..Default::default()
        };

        let (graph, out_label) = build_filter_graph(&timeline, &config).unwrap();
        let fc = graph.build();
        assert!(fc.contains("xfade=transition=dissolve"), "got: {fc}");
        assert_eq!(out_label, "xf1");
    }

    #[test]
    fn build_graph_with_text_overlay() {
        let mut timeline = empty_timeline(vec![make_clip("/v0.mp4", 10000)]);
        timeline.text_overlays.push(ResolvedTextOverlay {
            text: "Hello".into(),
            fontfile: None,
            fontsize: 48,
            fontcolor: "white".into(),
            x: "(w-text_w)/2".into(),
            y: "h-80".into(),
            start_ms: 1000,
            end_ms: 5000,
        });
        let config = RenderConfig {
            output_width: Some(1920),
            output_height: Some(1080),
            ..Default::default()
        };

        let (graph, out_label) = build_filter_graph(&timeline, &config).unwrap();
        let fc = graph.build();
        assert!(fc.contains("drawtext=text='Hello'"), "got: {fc}");
        assert!(fc.contains("between(t,1,5)"), "got: {fc}");
        assert_eq!(out_label, "txt0");
    }

    #[test]
    fn build_graph_with_ken_burns() {
        let mut clip = make_clip("/v0.mp4", 4000);
        clip.ken_burns = Some(KenBurnsParams {
            zoom_expr: "1+0.001*in".into(),
            x_expr: "iw/2-(iw/zoom/2)".into(),
            y_expr: "ih/2-(ih/zoom/2)".into(),
        });
        let timeline = empty_timeline(vec![clip]);
        let config = RenderConfig {
            output_width: Some(1920),
            output_height: Some(1080),
            output_fps: Some(30.0),
            ..Default::default()
        };

        let (graph, out_label) = build_filter_graph(&timeline, &config).unwrap();
        let fc = graph.build();
        assert!(fc.contains("zoompan=z='1+0.001*in'"), "got: {fc}");
        assert!(fc.contains("s=1920x1080"), "got: {fc}");
        assert_eq!(out_label, "kb0");
    }

    #[test]
    fn build_graph_with_color_effect() {
        let mut clip = make_clip("/v0.mp4", 5000);
        clip.color_effect = Some(ColorEffectParams::Eq {
            brightness: 0.05,
            contrast: 1.2,
            saturation: 1.5,
        });
        let timeline = empty_timeline(vec![clip]);
        let config = RenderConfig {
            output_width: Some(1920),
            output_height: Some(1080),
            ..Default::default()
        };

        let (graph, out_label) = build_filter_graph(&timeline, &config).unwrap();
        let fc = graph.build();
        assert!(fc.contains("eq=brightness=0.05"), "got: {fc}");
        assert_eq!(out_label, "ce0");
    }
}
