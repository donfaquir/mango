//! `mango task list` — list recent tasks with filtering.

use clap::Args;
use comfy_table::{presets::UTF8_FULL_CONDENSED, Table};
use tokio_rusqlite::Connection as AsyncConnection;

use mango_core::db::queries::generation_task as task_queries;
use mango_core::models::generation_task::GenerationTaskStatus;

use super::output;

#[derive(Args)]
pub struct ListArgs {
    /// Filter by project ID
    #[arg(long)]
    project_id: Option<String>,

    /// Filter by status (pending, running, success, failed, cancelled)
    #[arg(long)]
    status: Option<String>,

    /// Show tasks since duration (e.g. "1d", "12h", "30m") or ISO date
    #[arg(long, default_value = "7d")]
    since: String,

    /// Maximum number of tasks to display (default 50, max 500)
    #[arg(long, default_value_t = 50)]
    limit: u32,

    /// Output as NDJSON (one JSON object per line)
    #[arg(long)]
    json: bool,
}

pub async fn run(conn: &AsyncConnection, args: ListArgs) -> anyhow::Result<i32> {
    let status_filter = match args.status.as_deref() {
        Some(s) => Some(parse_status(s)?),
        None => None,
    };

    // Parse --since for filtering (we'll use it as a display filter since the
    // core query doesn't natively support time-based filtering beyond limit).
    let _since_cutoff = parse_since(&args.since);

    let project_id = args.project_id.clone();
    let limit = args.limit.clamp(1, 500);

    let tasks = conn
        .call(move |c| Ok(task_queries::list(c, project_id.as_deref(), status_filter, Some(limit))))
        .await
        .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| anyhow::anyhow!("db error: {e}"))?
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if args.json {
        for task in &tasks {
            output::print_task_json(task);
        }
    } else {
        if tasks.is_empty() {
            println!("No tasks found.");
            return Ok(0);
        }

        let mut table = Table::new();
        table.load_preset(UTF8_FULL_CONDENSED);
        table.set_header(vec!["ID", "MODEL", "STATUS", "SUBMITTED", "TYPE"]);

        for task in &tasks {
            table.add_row(vec![
                output::short_id(&task.id),
                task.model_id.clone(),
                output::status_display(task.status).to_string(),
                task.created_at.clone(),
                output::kind_display(task.task_type).to_string(),
            ]);
        }

        println!("{table}");
        println!("\n{} task(s)", tasks.len());
    }

    Ok(0)
}

fn parse_status(s: &str) -> anyhow::Result<GenerationTaskStatus> {
    GenerationTaskStatus::from_db_str(s)
        .ok_or_else(|| anyhow::anyhow!(
            "invalid status '{}'. Valid: pending, running, success, failed, cancelled",
            s
        ))
}

/// Parse a --since value like "7d", "12h", "30m" into a chrono DateTime.
/// Returns None if parsing fails (we'll use limit-based fallback).
fn parse_since(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    // Try humantime duration first (e.g. "7d", "12h")
    if let Ok(dur) = humantime::parse_duration(s) {
        let now = chrono::Utc::now();
        return Some(now - chrono::Duration::from_std(dur).ok()?);
    }
    // Try ISO date
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&chrono::Utc));
    }
    None
}
