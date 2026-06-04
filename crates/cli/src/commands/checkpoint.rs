use std::io::{self, Write};

use clap::{Args, Subcommand};
use comfy_table::{Table, presets::UTF8_FULL_CONDENSED};
use rusqlite::Connection;

use mango_core::db::queries::episode_checkpoint as checkpoint_queries;
use mango_core::db::queries::episode_checkpoint_ops as checkpoint_ops;
use mango_core::models::episode_checkpoint::CreateCheckpointInput;

#[derive(Args)]
pub struct CheckpointArgs {
    #[command(subcommand)]
    action: CheckpointAction,
}

#[derive(Subcommand)]
enum CheckpointAction {
    /// List checkpoints for an episode
    List {
        #[arg(long)]
        episode_id: String,
    },
    /// Create a manual checkpoint
    Create {
        #[arg(long)]
        episode_id: String,
        #[arg(long)]
        label: Option<String>,
    },
    /// Restore a checkpoint (creates auto-backup first)
    Restore {
        id: String,
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
    /// Delete a checkpoint
    Delete {
        id: String,
        /// Skip the confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

pub fn execute(conn: &mut Connection, args: CheckpointArgs) -> anyhow::Result<()> {
    match args.action {
        CheckpointAction::List { episode_id } => list(conn, &episode_id),
        CheckpointAction::Create { episode_id, label } => create(conn, &episode_id, label),
        CheckpointAction::Restore { id, yes } => restore(conn, &id, yes),
        CheckpointAction::Delete { id, yes } => delete(conn, &id, yes),
    }
}

fn list(conn: &Connection, episode_id: &str) -> anyhow::Result<()> {
    let rows = checkpoint_queries::list(conn, episode_id)?;
    if rows.is_empty() {
        println!("No checkpoints for episode {}", short_id(episode_id));
        return Ok(());
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED);
    table.set_header(vec!["ID", "Ver", "Type", "Label", "Summary", "Created"]);
    for cp in &rows {
        table.add_row(vec![
            short_id(&cp.id),
            cp.version_number.to_string(),
            cp.trigger_type.clone(),
            cp.label.clone().unwrap_or_default(),
            truncate(&cp.change_summary.clone().unwrap_or_default(), 30),
            cp.created_at.clone(),
        ]);
    }
    println!("{table}");
    println!("\n{} checkpoint(s) total", rows.len());
    Ok(())
}

fn create(conn: &Connection, episode_id: &str, label: Option<String>) -> anyhow::Result<()> {
    let cp = checkpoint_queries::insert_full(
        conn,
        &CreateCheckpointInput {
            episode_id: episode_id.to_string(),
            label,
            trigger_type: None,
        },
    )?;
    println!("Checkpoint created");
    println!("  ID:      {}", short_id(&cp.id));
    println!("  Version: {}", cp.version_number);
    if let Some(ref l) = cp.label {
        println!("  Label:   {l}");
    }
    if let Some(ref s) = cp.change_summary {
        println!("  Summary: {s}");
    }
    Ok(())
}

fn restore(conn: &mut Connection, id: &str, skip_confirm: bool) -> anyhow::Result<()> {
    if !skip_confirm {
        eprint!(
            "Restore checkpoint {}? This will overwrite current episode data. [y/N] ",
            short_id(id)
        );
        io::stderr().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }
    let episode = checkpoint_ops::restore(conn, id)?;
    println!(
        "Restored episode {} to checkpoint {}",
        short_id(&episode.id),
        short_id(id),
    );
    Ok(())
}

fn delete(conn: &Connection, id: &str, skip_confirm: bool) -> anyhow::Result<()> {
    if !skip_confirm {
        eprint!("Delete checkpoint {}? [y/N] ", short_id(id));
        io::stderr().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }
    checkpoint_queries::delete(conn, id)?;
    println!("Checkpoint deleted");
    Ok(())
}

fn short_id(id: &str) -> String {
    let n = 8.min(id.len());
    id[..n].to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max.saturating_sub(3)).collect();
    format!("{truncated}...")
}
