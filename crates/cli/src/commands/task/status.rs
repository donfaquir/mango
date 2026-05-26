//! `mango task status <id>` — show task details, with optional --watch polling.

use std::time::Duration;

use clap::Args;
use tokio_rusqlite::Connection as AsyncConnection;

use mango_core::db::queries::generation_task as task_queries;
use mango_core::models::generation_task::GenerationTaskStatus;

use super::output;

#[derive(Args)]
pub struct StatusArgs {
    /// Task ID to query
    task_id: String,

    /// Poll every 5s until task reaches terminal state
    #[arg(long)]
    watch: bool,

    /// Output JSON
    #[arg(long)]
    json: bool,
}

pub async fn run(conn: &AsyncConnection, args: StatusArgs) -> anyhow::Result<i32> {
    let task_id = args.task_id.clone();
    let task = conn
        .call(move |c| Ok(task_queries::get_by_id(c, &task_id)))
        .await
        .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| anyhow::anyhow!("db error: {e}"))?
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if args.json {
        output::print_task_json(&task);
    } else {
        output::print_task_detail(&task);
    }

    if !args.watch || task.status.is_terminal() {
        return Ok(0);
    }

    // Watch mode: poll until terminal
    let mut last_status = task.status;
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;

        let tid = args.task_id.clone();
        let task = conn
            .call(move |c| Ok(task_queries::get_by_id(c, &tid)))
            .await
            .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| anyhow::anyhow!("db error: {e}"))?
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        if task.status != last_status {
            if args.json {
                output::print_task_json(&task);
            } else {
                eprintln!(
                    "{} {}",
                    output::status_symbol(task.status),
                    output::status_display(task.status)
                );
            }
            last_status = task.status;
        }

        if task.status.is_terminal() {
            if !args.json {
                // Print final detail
                println!();
                output::print_task_detail(&task);
            }
            return Ok(match task.status {
                GenerationTaskStatus::Success => 0,
                GenerationTaskStatus::Failed => 10,
                GenerationTaskStatus::Cancelled => 11,
                _ => 0,
            });
        }
    }
}
