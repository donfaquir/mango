//! CLI `task` subcommand family: submit / status / list / cancel.
//!
//! The task subcommand starts its own tokio runtime and opens an async DB
//! connection. This avoids mixing sync/async on the same connection handle
//! and matches the engine's expectation of `tokio_rusqlite::Connection`.

mod cancel;
mod list;
mod output;
mod status;
mod submit;

use std::path::Path;

use clap::{Args, Subcommand};
use rusqlite::Connection;

#[derive(Args)]
pub struct TaskArgs {
    #[command(subcommand)]
    action: TaskAction,
}

#[derive(Subcommand)]
pub enum TaskAction {
    /// Submit a new generation task
    Submit(submit::SubmitArgs),
    /// Show task details (use --watch to follow)
    Status(status::StatusArgs),
    /// List recent tasks
    List(list::ListArgs),
    /// Cancel a running task
    Cancel(cancel::CancelArgs),
}

pub fn execute(_conn: &Connection, app_data_dir: &Path, args: TaskArgs) -> anyhow::Result<()> {
    // Ignore the parent's sync conn; task commands open their own async connection.
    let db_path = app_data_dir.join("mango.db");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    let exit_code = runtime.block_on(async move {
        let conn = mango_core::db::open_async(&db_path).await.map_err(|e| {
            anyhow::anyhow!("failed to open database at {}: {e}", db_path.display())
        })?;

        match args.action {
            TaskAction::Submit(a) => submit::run(&conn, app_data_dir, a).await,
            TaskAction::Status(a) => status::run(&conn, a).await,
            TaskAction::List(a) => list::run(&conn, a).await,
            TaskAction::Cancel(a) => cancel::run(&conn, a).await,
        }
    })?;

    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}
