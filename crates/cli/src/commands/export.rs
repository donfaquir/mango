use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use rusqlite::Connection;

use mango_core::app_config;
use mango_core::export;
use mango_core::ffmpeg::sidecar::FfmpegConfig;

#[derive(Args)]
pub struct ExportArgs {
    #[command(subcommand)]
    action: ExportAction,
}

#[derive(Subcommand)]
enum ExportAction {
    /// Export video clips for an episode (trim + concat)
    Clip {
        /// Episode ID whose clips to export
        #[arg(long)]
        episode_id: String,

        /// Output file path (e.g. ./output.mp4)
        #[arg(long)]
        output: PathBuf,
    },
    /// Export multi-track timeline (with transitions, effects, etc.)
    Timeline {
        /// Episode ID to export
        #[arg(long)]
        episode_id: String,

        /// Output file path (e.g. ./output.mp4)
        #[arg(long)]
        output: PathBuf,

        /// Encoding preset (ultrafast, fast, medium, slow)
        #[arg(long, default_value = "fast")]
        preset: String,

        /// Constant Rate Factor (0-51, lower = higher quality)
        #[arg(long, default_value = "18")]
        crf: u32,

        /// Video codec (libx264, libx265)
        #[arg(long, default_value = "libx264")]
        codec: String,

        /// Output width in pixels
        #[arg(long)]
        width: Option<i32>,

        /// Output height in pixels
        #[arg(long)]
        height: Option<i32>,

        /// Output frame rate
        #[arg(long)]
        fps: Option<f64>,

        /// Fit mode: crop, pad, or stretch
        #[arg(long)]
        fit_mode: Option<String>,

        /// Platform preset (douyin, bilibili, wechat_h, wechat_v, xiaohongshu)
        #[arg(long)]
        platform: Option<String>,

        /// Cover frame extraction time in milliseconds
        #[arg(long)]
        cover_time: Option<i64>,

        /// Cover frame output path (e.g. ./cover.jpg)
        #[arg(long)]
        cover_output: Option<PathBuf>,
    },
    /// Export final video with audio (voice + sfx + bgm)
    Final {
        /// Episode ID to export
        #[arg(long)]
        episode_id: String,

        /// Output file path (e.g. ./output.mp4)
        #[arg(long)]
        output: PathBuf,

        /// Include voice tracks
        #[arg(long, default_value = "true")]
        voice: bool,

        /// Include sound effect tracks
        #[arg(long, default_value = "true")]
        sfx: bool,

        /// Include background music tracks
        #[arg(long, default_value = "true")]
        bgm: bool,
    },
}

pub fn execute(conn: &Connection, app_data_dir: &Path, args: ExportArgs) -> anyhow::Result<()> {
    match args.action {
        ExportAction::Clip { episode_id, output } => clip(conn, app_data_dir, &episode_id, &output),
        ExportAction::Timeline {
            episode_id, output, preset, crf, codec,
            width, height, fps, fit_mode, platform,
            cover_time, cover_output,
        } => {
            timeline_export(
                conn, app_data_dir, &episode_id, &output, &preset, crf, &codec,
                width, height, fps, fit_mode.as_deref(), platform.as_deref(),
                cover_time, cover_output.as_deref(),
            )
        }
        ExportAction::Final {
            episode_id,
            output,
            voice,
            sfx,
            bgm,
        } => final_export(conn, app_data_dir, &episode_id, &output, voice, sfx, bgm),
    }
}

fn clip(
    conn: &Connection,
    app_data_dir: &Path,
    episode_id: &str,
    output: &Path,
) -> anyhow::Result<()> {
    let config = app_config::read(app_data_dir)?
        .ok_or_else(|| anyhow::anyhow!("no workspace configured — open the desktop app first to set up a workspace"))?;
    let workspace_root = &config.workspace_path;

    let resolved = export::resolve_clips(conn, episode_id, workspace_root)?;
    eprintln!("Resolved {} clip(s), starting export...", resolved.len());

    let ffmpeg_config = FfmpegConfig::from_env();
    let mut cb = |p: mango_core::ffmpeg::progress::FfmpegProgress| {
        eprint!("\r  exporting... {:.0}%", p.progress_pct);
        let _ = std::io::stderr().flush();
    };

    let result = export::export_resolved_clips(
        &ffmpeg_config,
        &resolved,
        output,
        Some(&mut cb),
    )?;

    eprintln!();

    let size = std::fs::metadata(&result)
        .map(|m| m.len())
        .unwrap_or(0);
    let size_display = if size >= 1_048_576 {
        format!("{:.1} MB", size as f64 / 1_048_576.0)
    } else {
        format!("{:.0} KB", size as f64 / 1024.0)
    };

    println!("{}", result.display());
    eprintln!("Export complete ({size_display})");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn timeline_export(
    conn: &Connection,
    app_data_dir: &Path,
    episode_id: &str,
    output: &Path,
    preset: &str,
    crf: u32,
    codec: &str,
    width: Option<i32>,
    height: Option<i32>,
    fps: Option<f64>,
    fit_mode: Option<&str>,
    platform: Option<&str>,
    cover_time: Option<i64>,
    cover_output: Option<&Path>,
) -> anyhow::Result<()> {
    let config = app_config::read(app_data_dir)?
        .ok_or_else(|| anyhow::anyhow!("no workspace configured"))?;
    let workspace_root = &config.workspace_path;
    let ffmpeg_config = FfmpegConfig::from_env();

    let mut render_config = if let Some(pid) = platform {
        let p = mango_core::aspect_ratio::get_platform_preset(pid)
            .ok_or_else(|| anyhow::anyhow!("unknown platform preset: {pid}"))?;
        mango_core::ffmpeg::render::RenderConfig {
            video_codec: p.codec.to_string(),
            preset: p.preset.to_string(),
            crf: p.crf,
            output_width: Some(p.width as i32),
            output_height: Some(p.height as i32),
            ..Default::default()
        }
    } else {
        mango_core::ffmpeg::render::RenderConfig {
            video_codec: codec.to_string(),
            preset: preset.to_string(),
            crf,
            ..Default::default()
        }
    };

    if let Some(w) = width { render_config.output_width = Some(w); }
    if let Some(h) = height { render_config.output_height = Some(h); }
    if let Some(f) = fps { render_config.output_fps = Some(f); }
    if let Some(fm) = fit_mode { render_config.fit_mode = Some(fm.to_string()); }

    eprintln!("Resolving timeline...");
    let mut cb = |p: mango_core::ffmpeg::progress::FfmpegProgress| {
        eprint!("\r  exporting... {:.0}%", p.progress_pct);
        let _ = std::io::stderr().flush();
    };

    let result = export::export_timeline(
        conn,
        episode_id,
        workspace_root,
        output,
        &render_config,
        &ffmpeg_config,
        Some(&mut cb),
    )?;

    eprintln!();

    if let (Some(time_ms), Some(cover_path)) = (cover_time, cover_output) {
        eprintln!("Extracting cover frame at {time_ms}ms...");
        mango_core::ffmpeg::commands::extract_thumbnail(
            &ffmpeg_config, &result, time_ms, cover_path,
        )?;
        eprintln!("Cover frame saved to {}", cover_path.display());
    }

    let size = std::fs::metadata(&result).map(|m| m.len()).unwrap_or(0);
    let size_display = if size >= 1_048_576 {
        format!("{:.1} MB", size as f64 / 1_048_576.0)
    } else {
        format!("{:.0} KB", size as f64 / 1024.0)
    };
    println!("{}", result.display());
    eprintln!("Export complete ({size_display})");
    Ok(())
}

fn final_export(
    conn: &Connection,
    app_data_dir: &Path,
    episode_id: &str,
    output: &Path,
    voice: bool,
    sfx: bool,
    bgm: bool,
) -> anyhow::Result<()> {
    let config = app_config::read(app_data_dir)?
        .ok_or_else(|| anyhow::anyhow!("no workspace configured — open the desktop app first to set up a workspace"))?;
    let workspace_root = &config.workspace_path;

    let ffmpeg_config = FfmpegConfig::from_env();
    let settings = export::FinalExportSettings {
        include_voice: voice,
        include_sfx: sfx,
        include_bgm: bgm,
    };

    let resolved = export::resolve_final_export(
        conn,
        episode_id,
        workspace_root,
        &settings,
        &ffmpeg_config,
    )?;
    eprintln!(
        "Resolved {} clip(s) + {} audio track(s), starting export...",
        resolved.clips.len(),
        resolved.audio_tracks.len()
    );

    let mut cb = |p: mango_core::ffmpeg::progress::FfmpegProgress| {
        eprint!("\r  exporting... {:.0}%", p.progress_pct);
        let _ = std::io::stderr().flush();
    };

    let result = export::export_final(
        &ffmpeg_config,
        &resolved,
        output,
        Some(&mut cb),
    )?;

    eprintln!();

    let size = std::fs::metadata(&result)
        .map(|m| m.len())
        .unwrap_or(0);
    let size_display = if size >= 1_048_576 {
        format!("{:.1} MB", size as f64 / 1_048_576.0)
    } else {
        format!("{:.0} KB", size as f64 / 1024.0)
    };

    println!("{}", result.display());
    eprintln!("Export complete ({size_display})");
    Ok(())
}
