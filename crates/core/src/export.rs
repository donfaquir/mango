use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use crate::db::queries::{asset, episode, project, shot_audio, video_clip};
use crate::error::{CoreError, Result};
use crate::ken_burns;
use crate::ffmpeg::audio::AudioMixInput;
use crate::ffmpeg::commands::TrimMode;
use crate::ffmpeg::progress::FfmpegProgress;
use crate::ffmpeg::sidecar::FfmpegConfig;
use crate::models::shot_audio::AudioRole;

fn resolve_project_root(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
) -> Result<PathBuf> {
    let ep = episode::get_by_id(conn, episode_id)?;
    let proj = project::get_by_id(conn, &ep.project_id)?;
    let root = Path::new(&proj.root_path);
    if root.is_absolute() {
        Ok(root.to_path_buf())
    } else {
        Ok(workspace_root.join(root))
    }
}

#[derive(Debug)]
pub struct ResolvedClip {
    pub source_path: PathBuf,
    pub trim_start_ms: Option<i64>,
    pub trim_end_ms: Option<i64>,
}

pub fn resolve_clips(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
) -> Result<Vec<ResolvedClip>> {
    let project_root = resolve_project_root(conn, episode_id, workspace_root)?;
    let clips = video_clip::list(conn, episode_id)?;
    if clips.is_empty() {
        return Err(CoreError::Validation(
            "no video clips to export for this episode".into(),
        ));
    }

    clips
        .into_iter()
        .map(|clip| {
            let a = asset::get_by_id(conn, &clip.source_asset_id)?;
            let abs_path = project_root.join(&a.file_path);
            if !abs_path.exists() {
                return Err(CoreError::Validation(format!(
                    "asset file not found: {}",
                    abs_path.display()
                )));
            }
            Ok(ResolvedClip {
                source_path: abs_path,
                trim_start_ms: clip.trim_start_ms,
                trim_end_ms: clip.trim_end_ms,
            })
        })
        .collect()
}

pub fn export_resolved_clips(
    config: &FfmpegConfig,
    clips: &[ResolvedClip],
    output_path: &Path,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf> {
    if clips.is_empty() {
        return Err(CoreError::Validation(
            "no video clips to export".into(),
        ));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if clips.len() == 1 {
        let clip = &clips[0];
        let no_trim = clip.trim_start_ms.is_none() && clip.trim_end_ms.is_none();
        if no_trim {
            std::fs::copy(&clip.source_path, output_path)?;
        } else {
            let start = clip.trim_start_ms.unwrap_or(0);
            let end = match clip.trim_end_ms {
                Some(e) => e,
                None => crate::ffmpeg::probe::probe_video(config, &clip.source_path)?.duration_ms,
            };
            crate::ffmpeg::commands::trim_video(
                config,
                &clip.source_path,
                start,
                end,
                output_path,
                &TrimMode::Copy,
                on_progress,
            )?;
        }
        return Ok(output_path.to_path_buf());
    }

    let temp_dir = std::env::temp_dir().join(format!("mango_export_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let mut trimmed_paths: Vec<PathBuf> = Vec::with_capacity(clips.len());
    for (i, clip) in clips.iter().enumerate() {
        let needs_trim = clip.trim_start_ms.is_some() || clip.trim_end_ms.is_some();
        if needs_trim {
            let start = clip.trim_start_ms.unwrap_or(0);
            let end = match clip.trim_end_ms {
                Some(e) => e,
                None => {
                    crate::ffmpeg::probe::probe_video(config, &clip.source_path)?.duration_ms
                }
            };
            let trimmed = temp_dir.join(format!("trim_{i}.mp4"));
            crate::ffmpeg::commands::trim_video(
                config,
                &clip.source_path,
                start,
                end,
                &trimmed,
                &TrimMode::Copy,
                None,
            )?;
            trimmed_paths.push(trimmed);
        } else {
            trimmed_paths.push(clip.source_path.clone());
        }
    }

    crate::ffmpeg::commands::concat_videos(config, &trimmed_paths, output_path, on_progress)?;

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(output_path.to_path_buf())
}

// ─── final export (video + audio) ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct FinalExportSettings {
    #[serde(default = "default_true")]
    pub include_voice: bool,
    #[serde(default = "default_true")]
    pub include_sfx: bool,
    #[serde(default = "default_true")]
    pub include_bgm: bool,
}

fn default_true() -> bool {
    true
}

impl Default for FinalExportSettings {
    fn default() -> Self {
        Self {
            include_voice: true,
            include_sfx: true,
            include_bgm: true,
        }
    }
}

#[derive(Debug)]
pub struct ResolvedAudio {
    pub path: PathBuf,
    pub role: AudioRole,
    pub volume: f64,
    pub absolute_offset_ms: i64,
}

#[derive(Debug)]
pub struct ResolvedFinalExport {
    pub clips: Vec<ResolvedClip>,
    pub audio_tracks: Vec<ResolvedAudio>,
    pub total_video_duration_ms: i64,
}

pub fn resolve_final_export(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
    settings: &FinalExportSettings,
    config: &FfmpegConfig,
) -> Result<ResolvedFinalExport> {
    let project_root = resolve_project_root(conn, episode_id, workspace_root)?;
    let clips = resolve_clips(conn, episode_id, workspace_root)?;

    let mut clip_durations: Vec<i64> = Vec::with_capacity(clips.len());
    for clip in &clips {
        let meta = crate::ffmpeg::probe::probe_video(config, &clip.source_path)?;
        let start = clip.trim_start_ms.unwrap_or(0);
        let end = clip.trim_end_ms.unwrap_or(meta.duration_ms);
        clip_durations.push(end - start);
    }
    let total_video_duration_ms: i64 = clip_durations.iter().sum();

    let video_clips = video_clip::list(conn, episode_id)?;

    let mut audio_tracks: Vec<ResolvedAudio> = Vec::new();
    let mut timeline_offset: i64 = 0;

    for (i, vc) in video_clips.iter().enumerate() {
        let source_asset = asset::get_by_id(conn, &vc.source_asset_id)?;
        let shot_id = match source_asset.shot_id {
            Some(ref id) => id.clone(),
            None => {
                timeline_offset += clip_durations[i];
                continue;
            }
        };

        let bindings = shot_audio::list_by_shot(conn, &shot_id)?;
        for binding in &bindings {
            let include = match binding.audio_role {
                AudioRole::Voice => settings.include_voice,
                AudioRole::Sfx => settings.include_sfx,
                AudioRole::Bgm => settings.include_bgm,
            };
            if !include {
                continue;
            }

            let audio_asset = asset::get_by_id(conn, &binding.asset_id)?;
            let audio_path = project_root.join(&audio_asset.file_path);
            if !audio_path.exists() {
                tracing::warn!(
                    "skipping shot_audio {}: file not found at {}",
                    binding.id,
                    audio_path.display()
                );
                continue;
            }

            audio_tracks.push(ResolvedAudio {
                path: audio_path,
                role: binding.audio_role,
                volume: binding.volume,
                absolute_offset_ms: timeline_offset + binding.offset_ms,
            });
        }

        timeline_offset += clip_durations[i];
    }

    Ok(ResolvedFinalExport {
        clips,
        audio_tracks,
        total_video_duration_ms,
    })
}

pub fn export_final(
    config: &FfmpegConfig,
    resolved: &ResolvedFinalExport,
    output_path: &Path,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf> {
    if resolved.clips.is_empty() {
        return Err(CoreError::Validation("no video clips to export".into()));
    }

    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if resolved.audio_tracks.is_empty() {
        return export_resolved_clips(config, &resolved.clips, output_path, on_progress);
    }

    let temp_dir = std::env::temp_dir().join(format!("mango_final_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir)?;

    let video_only = temp_dir.join("video_assembled.mp4");
    export_resolved_clips(config, &resolved.clips, &video_only, None)?;

    let audio_inputs: Vec<AudioMixInput> = resolved
        .audio_tracks
        .iter()
        .map(|t| AudioMixInput {
            path: t.path.clone(),
            volume: t.volume,
            offset_ms: t.absolute_offset_ms,
        })
        .collect();

    crate::ffmpeg::audio::overlay_audio_on_video(
        config,
        &video_only,
        &audio_inputs,
        output_path,
        on_progress,
    )?;

    let _ = std::fs::remove_dir_all(&temp_dir);
    Ok(output_path.to_path_buf())
}

// ─── timeline export (multi-track) ───────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ExportMode {
    Copy,
    Render,
}

pub fn resolve_timeline(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
    config: &FfmpegConfig,
) -> Result<(crate::ffmpeg::render::ResolvedTimeline, ExportMode)> {
    use crate::db::queries::{timeline_item, timeline_track};
    use crate::ffmpeg::render::*;
    use crate::models::timeline::TrackType;
    use crate::models::timeline_params::{
        default_text_style, BubbleStyle, TextStyle, TextType,
    };

    let project_root = resolve_project_root(conn, episode_id, workspace_root)?;
    let tracks = timeline_track::list_by_episode(conn, episode_id)?;
    if tracks.is_empty() {
        return Err(CoreError::Validation(
            "no timeline tracks for this episode".into(),
        ));
    }

    let all_items = timeline_item::list_by_episode(conn, episode_id)?;
    let video_items: Vec<_> = all_items
        .iter()
        .filter(|i| {
            tracks
                .iter()
                .find(|t| t.id == i.track_id)
                .is_some_and(|t| t.track_type == TrackType::Video)
        })
        .collect();

    if video_items.is_empty() {
        return Err(CoreError::Validation("no video clips in timeline".into()));
    }

    let mut clips = Vec::new();
    let mut has_effects = false;

    for item in &video_items {
        if item.item_type != crate::models::timeline::ItemType::Clip {
            has_effects = true;
            continue;
        }
        let asset_id = item
            .asset_id
            .as_deref()
            .ok_or_else(|| CoreError::Validation("video clip item missing asset_id".into()))?;
        let a = asset::get_by_id(conn, asset_id)?;
        let abs_path = project_root.join(&a.file_path);
        if !abs_path.exists() {
            return Err(CoreError::Validation(format!(
                "asset file not found: {}",
                abs_path.display()
            )));
        }

        let ken_burns_preset_id = match crate::models::timeline_params::parse_params(&item.params_json) {
            Ok(crate::models::timeline_params::TimelineItemParams::Clip {
                ken_burns_preset: Some(ref preset_id),
            }) if ken_burns::get_preset(preset_id).is_some() => {
                has_effects = true;
                Some(preset_id.clone())
            }
            _ => None,
        };

        clips.push(ResolvedTimelineClip {
            source_path: abs_path,
            position_ms: item.position_ms,
            duration_ms: item.duration_ms,
            in_point_ms: item.in_point_ms,
            out_point_ms: item.out_point_ms,
            ken_burns: None,
            ken_burns_preset_id,
            color_effect: None,
        });
    }

    let transitions: Vec<ResolvedTransition> = all_items
        .iter()
        .filter(|i| i.item_type == crate::models::timeline::ItemType::Transition)
        .map(|i| {
            has_effects = true;
            let tt = serde_json::from_str::<serde_json::Value>(&i.params_json)
                .ok()
                .and_then(|v| v["transition_type"].as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "dissolve".into());
            ResolvedTransition {
                transition_type: tt,
                duration_ms: i.duration_ms,
                position_ms: i.position_ms,
            }
        })
        .collect();

    let text_overlays: Vec<ResolvedTextOverlay> = all_items
        .iter()
        .filter(|i| {
            i.item_type == crate::models::timeline::ItemType::Text
                && tracks
                    .iter()
                    .find(|t| t.id == i.track_id)
                    .is_some_and(|t| t.track_type == TrackType::Text)
        })
        .map(|i| {
            has_effects = true;
            let v = serde_json::from_str::<serde_json::Value>(&i.params_json).unwrap_or_default();
            let content = v["content"].as_str().unwrap_or("").to_string();
            let text_type = match v["text_type"].as_str().unwrap_or("subtitle") {
                "bubble" => TextType::Bubble,
                "fancy" => TextType::Fancy,
                "onomatopoeia" => TextType::Onomatopoeia,
                _ => TextType::Subtitle,
            };
            let defaults = default_text_style(&text_type);
            let style: TextStyle = v
                .get("style")
                .and_then(|s| serde_json::from_value(s.clone()).ok())
                .unwrap_or_default();

            let font_size = style.font_size.or(defaults.font_size).unwrap_or(48);
            let color = style
                .color
                .as_deref()
                .or(defaults.color.as_deref())
                .unwrap_or("white")
                .to_string();
            let pos_x = style.position_x.or(defaults.position_x);
            let pos_y = style.position_y.or(defaults.position_y);
            let alignment = style
                .alignment
                .as_deref()
                .or(defaults.alignment.as_deref())
                .unwrap_or("center");

            let x_expr = match (pos_x, alignment) {
                (Some(px), "left") => format!("{px}*w"),
                (Some(px), "right") => format!("{px}*w-text_w"),
                (Some(px), _) => format!("{px}*w-text_w/2"),
                (None, _) => "(w-text_w)/2".to_string(),
            };
            let y_expr = pos_y
                .map(|py| format!("{py}*h"))
                .unwrap_or_else(|| "h-80".to_string());

            let borderw = style.outline_width.or(defaults.outline_width);
            let bordercolor = style
                .outline_color
                .clone()
                .or(defaults.outline_color.clone());
            let (shadowx, shadowy, shadowcolor) = if style.shadow.unwrap_or(false) {
                (Some(2), Some(2), Some("black@0.6".to_string()))
            } else {
                (None, None, None)
            };

            let bubble: Option<BubbleStyle> = v
                .get("bubble")
                .and_then(|b| serde_json::from_value(b.clone()).ok());
            let (boxcolor, boxborderw) = if matches!(text_type, TextType::Bubble) {
                let fill = bubble
                    .as_ref()
                    .and_then(|b| b.fill_color.as_deref())
                    .unwrap_or("white@0.9");
                (Some(fill.to_string()), Some(10u32))
            } else {
                (None, None)
            };

            ResolvedTextOverlay {
                text: content,
                fontfile: style.font_family.clone(),
                fontsize: font_size,
                fontcolor: color,
                x: x_expr,
                y: y_expr,
                start_ms: i.position_ms,
                end_ms: i.position_ms + i.duration_ms,
                borderw,
                bordercolor,
                shadowx,
                shadowy,
                shadowcolor,
                boxcolor,
                boxborderw,
            }
        })
        .collect();

    let audio_tracks: Vec<AudioMixInput> = all_items
        .iter()
        .filter(|i| {
            tracks
                .iter()
                .find(|t| t.id == i.track_id)
                .is_some_and(|t| t.track_type == TrackType::Audio && !t.muted)
        })
        .filter_map(|i| {
            let aid = i.asset_id.as_deref()?;
            let a = asset::get_by_id(conn, aid).ok()?;
            let abs_path = project_root.join(&a.file_path);
            if abs_path.exists() {
                Some(AudioMixInput {
                    path: abs_path,
                    volume: 1.0,
                    offset_ms: i.position_ms,
                })
            } else {
                None
            }
        })
        .collect();

    let total_duration_ms = clips
        .iter()
        .map(|c| c.position_ms + c.duration_ms)
        .max()
        .unwrap_or(0);

    let mode = if has_effects || !transitions.is_empty() || !text_overlays.is_empty() {
        ExportMode::Render
    } else {
        let first = &clips[0];
        let all_same = clips.iter().skip(1).all(|c| {
            let fa = crate::ffmpeg::probe::probe_video(config, &first.source_path).ok();
            let ca = crate::ffmpeg::probe::probe_video(config, &c.source_path).ok();
            match (fa, ca) {
                (Some(fm), Some(cm)) => {
                    fm.width == cm.width
                        && fm.height == cm.height
                        && (fm.fps - cm.fps).abs() < 0.01
                        && fm.video_codec == cm.video_codec
                }
                _ => false,
            }
        });
        if all_same {
            ExportMode::Copy
        } else {
            ExportMode::Render
        }
    };

    let timeline = crate::ffmpeg::render::ResolvedTimeline {
        clips,
        transitions,
        text_overlays,
        sticker_overlays: vec![],
        audio_tracks,
        total_duration_ms,
    };

    Ok((timeline, mode))
}

pub fn export_timeline(
    conn: &Connection,
    episode_id: &str,
    workspace_root: &Path,
    output_path: &Path,
    render_config: &crate::ffmpeg::render::RenderConfig,
    ffmpeg_config: &FfmpegConfig,
    on_progress: Option<&mut dyn FnMut(FfmpegProgress)>,
) -> Result<PathBuf> {
    let (timeline, mode) = resolve_timeline(conn, episode_id, workspace_root, ffmpeg_config)?;

    match mode {
        ExportMode::Copy => {
            let resolved: Vec<ResolvedClip> = timeline
                .clips
                .iter()
                .map(|c| ResolvedClip {
                    source_path: c.source_path.clone(),
                    trim_start_ms: if c.in_point_ms > 0 {
                        Some(c.in_point_ms)
                    } else {
                        None
                    },
                    trim_end_ms: if c.out_point_ms < c.duration_ms {
                        Some(c.out_point_ms)
                    } else {
                        None
                    },
                })
                .collect();
            export_resolved_clips(ffmpeg_config, &resolved, output_path, on_progress)
        }
        ExportMode::Render => {
            crate::ffmpeg::render::render_timeline(
                render_config,
                ffmpeg_config,
                &timeline,
                output_path,
                on_progress,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_sync;
    use crate::db::queries::video_clip;
    use crate::models::video_clip::CreateVideoClipInput;
    use std::path::Path;
    use tempfile::TempDir;

    fn setup() -> Connection {
        let conn = open_sync(Path::new(":memory:")).unwrap();
        conn.execute(
            "INSERT INTO project (id, name, root_path) VALUES ('p1', 'Test Project', 'projects/test')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO episode (id, project_id, title, order_index) VALUES ('e1', 'p1', 'Ep1', 0)",
            [],
        )
        .unwrap();
        conn
    }

    fn insert_asset(conn: &Connection, id: &str, file_path: &str) {
        conn.execute(
            "INSERT INTO asset (id, project_id, asset_type, file_path, file_size, original_name)
             VALUES (?1, 'p1', 'video', ?2, 1000, 'v.mp4')",
            rusqlite::params![id, file_path],
        )
        .unwrap();
    }

    #[test]
    fn resolve_no_clips_returns_error() {
        let conn = setup();
        let err = resolve_clips(&conn, "e1", Path::new("/tmp")).unwrap_err();
        assert!(
            err.to_string().contains("no video clips"),
            "expected validation error, got: {err}"
        );
    }

    #[test]
    fn resolve_clips_missing_file_returns_error() {
        let conn = setup();
        insert_asset(&conn, "a1", "assets/a1.mp4");
        video_clip::create(
            &conn,
            CreateVideoClipInput {
                project_id: "p1".into(),
                episode_id: Some("e1".into()),
                source_asset_id: "a1".into(),
                label: None,
                trim_start_ms: None,
                trim_end_ms: None,
            },
        )
        .unwrap();

        let err = resolve_clips(&conn, "e1", Path::new("/nonexistent")).unwrap_err();
        assert!(
            err.to_string().contains("not found"),
            "expected file-not-found error, got: {err}"
        );
    }

    #[test]
    fn resolve_clips_with_real_files() {
        let tmp = TempDir::new().unwrap();
        let video_path = "assets/v1.mp4";
        let abs = tmp.path().join("projects/test").join(video_path);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        std::fs::write(&abs, b"fake video content").unwrap();

        let conn = setup();
        insert_asset(&conn, "a1", video_path);
        video_clip::create(
            &conn,
            CreateVideoClipInput {
                project_id: "p1".into(),
                episode_id: Some("e1".into()),
                source_asset_id: "a1".into(),
                label: None,
                trim_start_ms: Some(1000),
                trim_end_ms: Some(5000),
            },
        )
        .unwrap();

        let clips = resolve_clips(&conn, "e1", tmp.path()).unwrap();
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].source_path, abs);
        assert_eq!(clips[0].trim_start_ms, Some(1000));
        assert_eq!(clips[0].trim_end_ms, Some(5000));
    }

    #[test]
    fn export_empty_clips_returns_error() {
        let config = FfmpegConfig {
            ffmpeg_path: None,
            ffprobe_path: None,
        };
        let err =
            export_resolved_clips(&config, &[], Path::new("/tmp/out.mp4"), None).unwrap_err();
        assert!(
            err.to_string().contains("no video clips"),
            "expected validation error, got: {err}"
        );
    }

    #[test]
    fn export_single_no_trim_copies_file() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("source.mp4");
        let content = b"fake mp4 data for testing copy path";
        std::fs::write(&src, content).unwrap();

        let output = tmp.path().join("output.mp4");
        let config = FfmpegConfig {
            ffmpeg_path: None,
            ffprobe_path: None,
        };

        let clip = ResolvedClip {
            source_path: src.clone(),
            trim_start_ms: None,
            trim_end_ms: None,
        };

        let result = export_resolved_clips(&config, &[clip], &output, None).unwrap();
        assert_eq!(result, output);
        assert_eq!(std::fs::read(&output).unwrap(), content);
    }
}
