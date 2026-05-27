//! `mango task status <id>` — show task details, with optional --watch polling.

use std::time::Duration;

use clap::Args;
use tokio_rusqlite::Connection as AsyncConnection;

use mango_core::db::queries::generation_task as task_queries;
use mango_core::db::queries::generation_task_event as event_queries;
use mango_core::models::generation_task::GenerationTaskStatus;
use mango_core::models::generation_task_event::GenerationTaskEvent;

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

    /// Print the full diagnostic event timeline
    #[arg(long)]
    verbose: bool,
}

pub async fn run(conn: &AsyncConnection, args: StatusArgs) -> anyhow::Result<i32> {
    let task_id = args.task_id.clone();
    let task = conn
        .call(move |c| Ok(task_queries::get_by_id(c, &task_id)))
        .await
        .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| anyhow::anyhow!("db error: {e}"))?
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    let events = if args.verbose {
        load_events(conn, &args.task_id).await?
    } else {
        Vec::new()
    };

    if args.json {
        if args.verbose {
            print_task_with_events_json(&task, &events);
        } else {
            output::print_task_json(&task);
        }
    } else {
        output::print_task_detail(&task);
        if args.verbose {
            print_event_timeline(&events);
        }
    }

    if !args.watch || task.status.is_terminal() {
        return Ok(0);
    }

    // Watch mode: poll until terminal. In --verbose --watch we re-fetch the
    // event timeline each tick and print rows we haven't seen yet.
    let mut last_status = task.status;
    let mut printed_event_ids: std::collections::HashSet<i64> =
        events.iter().map(|e| e.id).collect();
    loop {
        tokio::time::sleep(Duration::from_secs(5)).await;

        let tid = args.task_id.clone();
        let task = conn
            .call(move |c| Ok(task_queries::get_by_id(c, &tid)))
            .await
            .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| anyhow::anyhow!("db error: {e}"))?
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        if args.verbose {
            let new_events = load_events(conn, &args.task_id).await?;
            for ev in &new_events {
                if printed_event_ids.insert(ev.id) {
                    eprintln!("{}", output::format_event_line(ev));
                }
            }
        }

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

async fn load_events(
    conn: &AsyncConnection,
    task_id: &str,
) -> anyhow::Result<Vec<GenerationTaskEvent>> {
    let id = task_id.to_string();
    conn.call(move |c| Ok(event_queries::list_by_task(c, &id)))
        .await
        .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| anyhow::anyhow!("db error: {e}"))?
        .map_err(|e| anyhow::anyhow!("{e}"))
}

fn print_event_timeline(events: &[GenerationTaskEvent]) {
    println!();
    println!("Events ({}):", events.len());
    if events.is_empty() {
        println!("  (none)");
        return;
    }
    for ev in events {
        println!("  {}", output::format_event_line(ev));
    }
}

fn print_task_with_events_json(
    task: &mango_core::models::generation_task::GenerationTask,
    events: &[GenerationTaskEvent],
) {
    let task_value = serde_json::to_value(task).unwrap_or(serde_json::Value::Null);
    let events_value = serde_json::to_value(events).unwrap_or(serde_json::Value::Array(vec![]));
    let combined = serde_json::json!({
        "task": task_value,
        "events": events_value,
    });
    if let Ok(s) = serde_json::to_string(&combined) {
        println!("{s}");
    }
}
