//! `mango task cancel <id>` — cancel a running or pending task.

use std::io::Write;
use std::sync::Arc;

use clap::Args;
use tokio_rusqlite::Connection as AsyncConnection;

use mango_core::account::keyring::{CachedKeyringStore, KeyringStore, SystemKeyring};
use mango_core::db::queries::generation_task as task_queries;
use mango_core::provider::bailian::BailianProvider;
use mango_core::provider::ProviderRegistry;
use mango_core::task_engine::{NoopMaterializer, TaskEngineHandle};

use super::output;

#[derive(Args)]
pub struct CancelArgs {
    /// Task ID to cancel
    task_id: String,

    /// Skip confirmation prompt
    #[arg(long)]
    yes: bool,
}

pub async fn run(conn: &AsyncConnection, args: CancelArgs) -> anyhow::Result<i32> {
    // First, check task exists and its current status
    let task_id = args.task_id.clone();
    let task = conn
        .call(move |c| Ok(task_queries::get_by_id(c, &task_id)))
        .await
        .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| anyhow::anyhow!("db error: {e}"))?
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if task.status.is_terminal() {
        eprintln!(
            "Task {} is already in terminal state: {}",
            output::short_id(&task.id),
            output::status_display(task.status)
        );
        return Ok(0);
    }

    // Confirmation prompt unless --yes
    if !args.yes {
        eprint!(
            "Cancel task {} (currently {})? [y/N] ",
            output::short_id(&task.id),
            output::status_display(task.status)
        );
        std::io::stderr().flush()?;

        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            eprintln!("Aborted.");
            return Ok(0);
        }
    }

    // Initialize keyring for engine (cancel may need provider cancel)
    keyring::use_native_store(false)
        .map_err(|e| anyhow::anyhow!("failed to register native keyring store: {e}"))?;

    let keyring: Arc<dyn KeyringStore> =
        Arc::new(CachedKeyringStore::new(Box::new(SystemKeyring)));
    let providers = ProviderRegistry::builder()
        .register("bailian", Arc::new(BailianProvider::new()))
        .build();
    let materializer: Arc<dyn mango_core::task_engine::ResultMaterializer> =
        Arc::new(NoopMaterializer);

    let (engine, _rx) =
        TaskEngineHandle::spawn(conn.clone(), providers, keyring, materializer, 4);

    engine
        .cancel(&args.task_id)
        .await
        .map_err(|e| anyhow::anyhow!("cancel failed: {e}"))?;

    eprintln!(
        "Cancelled task {}",
        output::short_id(&task.id),
    );
    eprintln!(
        "  Was status: {} (will move to cancelled)",
        output::status_display(task.status)
    );

    Ok(0)
}
