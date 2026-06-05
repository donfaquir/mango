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
}

pub fn execute(conn: &Connection, app_data_dir: &Path, args: ExportArgs) -> anyhow::Result<()> {
    match args.action {
        ExportAction::Clip { episode_id, output } => clip(conn, app_data_dir, &episode_id, &output),
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
