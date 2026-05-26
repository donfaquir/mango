//! `mango task submit` — submit a new generation task.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context};
use clap::Args;
use tokio_rusqlite::Connection as AsyncConnection;

use mango_core::account::keyring::{KeyringStore, SystemKeyring};
use mango_core::db::queries::api_account as account_queries;
use mango_core::models::generation_task::{
    CreateGenerationTaskInput, GenerationTask, GenerationTaskStatus, TaskKind,
};
use mango_core::provider::bailian::materializer::BailianResultMaterializer;
use mango_core::provider::bailian::BailianProvider;
use mango_core::provider::ProviderRegistry;
use mango_core::task_engine::TaskEngineHandle;

use super::output;

#[derive(Args)]
pub struct SubmitArgs {
    /// Target project ID (required)
    #[arg(long)]
    project_id: Option<String>,

    /// Provider ID (default: bailian)
    #[arg(long, default_value = "bailian")]
    provider: String,

    /// Specific account ID (overrides --provider auto-selection)
    #[arg(long)]
    account_id: Option<String>,

    /// Model ID (required, e.g. wan2.7-image-pro, happyhorse-1.0-r2v)
    #[arg(long)]
    model: String,

    /// Prompt text
    #[arg(long, conflicts_with = "prompt_file")]
    prompt: Option<String>,

    /// Read prompt from file
    #[arg(long, conflicts_with = "prompt")]
    prompt_file: Option<String>,

    /// Block until task reaches terminal state
    #[arg(long)]
    wait: bool,

    /// Output JSON instead of human-readable text
    #[arg(long)]
    json: bool,

    // --- Model-specific options (wan2.7-image-pro) ---
    /// Number of images to generate (1-4, wan2.7 only)
    #[arg(long)]
    n: Option<u8>,

    /// Image size: 1K, 2K, 4K (wan2.7 only)
    #[arg(long)]
    size: Option<String>,

    /// Enable sequential consistency for multi-image (wan2.7 only)
    #[arg(long)]
    enable_sequential: bool,

    /// Negative prompt (wan2.7 only)
    #[arg(long)]
    negative_prompt: Option<String>,

    // --- Model-specific options (happyhorse-1.0-r2v) ---
    /// Subject ID (can repeat; happyhorse only)
    #[arg(long)]
    subject: Vec<String>,

    /// Video resolution: 720P, 1080P (happyhorse only)
    #[arg(long)]
    resolution: Option<String>,

    /// Aspect ratio: 16:9, 9:16, 1:1 (happyhorse only)
    #[arg(long)]
    ratio: Option<String>,

    /// Video duration in seconds: 3, 5, 10 (happyhorse only)
    #[arg(long)]
    duration: Option<u32>,
}

pub async fn run(
    conn: &AsyncConnection,
    _app_data_dir: &Path,
    args: SubmitArgs,
) -> anyhow::Result<i32> {
    // 1. Resolve prompt
    let prompt = resolve_prompt(&args)?;

    // 2. Resolve account_id
    let account_id = resolve_account(conn, &args).await?;

    // 3. Build params_json based on model
    let (task_type, params_json) = build_params(&args, &prompt)?;

    // 4. Initialize keyring + engine
    keyring::use_native_store(false)
        .map_err(|e| anyhow::anyhow!("failed to register native keyring store: {e}"))?;

    let engine = setup_engine(conn.clone()).await?;

    // 5. Submit via engine (creates task row + spawns runner)
    let input = CreateGenerationTaskInput {
        shot_id: None, // CLI submit without shot association for now
        provider_id: args.provider.clone(),
        model_id: args.model.clone(),
        account_id: account_id.clone(),
        task_type,
        params_json: Some(params_json),
    };

    let task_id = engine
        .submit(input)
        .await
        .map_err(|e| anyhow::anyhow!("submit failed: {e}"))?;

    // Fetch the created task for display
    let task = engine
        .get(&task_id)
        .await
        .map_err(|e| anyhow::anyhow!("failed to get task: {e}"))?;

    if args.wait {
        if !args.json {
            eprintln!(
                "{} {} submitted, waiting...",
                output::status_symbol(GenerationTaskStatus::Pending),
                output::short_id(&task.id)
            );
        }

        let code = wait_for_terminal(&engine, &task.id, args.json).await?;
        Ok(code)
    } else {
        // Print submission summary and hint about --wait
        if args.json {
            output::print_task_json(&task);
        } else {
            output::print_submitted(&task);
            eprintln!();
            eprintln!("Note: task is pending. Use --wait to block until completion,");
            eprintln!("or ensure the GUI is running to process it.");
        }
        Ok(0)
    }
}

fn resolve_prompt(args: &SubmitArgs) -> anyhow::Result<String> {
    match (&args.prompt, &args.prompt_file) {
        (Some(p), _) => Ok(p.clone()),
        (_, Some(path)) => {
            std::fs::read_to_string(path).with_context(|| format!("reading prompt file: {path}"))
        }
        (None, None) => bail!("either --prompt or --prompt-file is required"),
    }
}

async fn resolve_account(conn: &AsyncConnection, args: &SubmitArgs) -> anyhow::Result<String> {
    if let Some(id) = args.account_id.as_deref() {
        return Ok(id.to_string());
    }
    let provider = args.provider.clone();
    let accounts = conn
        .call(move |c| {
            Ok(account_queries::list(c, Some(provider))
                .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e))))
        })
        .await
        .map_err(|e: tokio_rusqlite::Error<rusqlite::Error>| anyhow::anyhow!("db error: {e}"))?
        .map_err(|e| anyhow::anyhow!("db error: {e}"))?;

    accounts
        .first()
        .map(|a| a.id.clone())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no account configured for provider '{}'. \
                 Run `mango account add --provider-id {} ...` first.",
                args.provider,
                args.provider
            )
        })
}

fn build_params(args: &SubmitArgs, prompt: &str) -> anyhow::Result<(TaskKind, String)> {
    match args.model.as_str() {
        "wan2.7-image-pro" => {
            if !args.subject.is_empty() {
                bail!("--subject is not applicable for wan2.7-image-pro");
            }
            let v = serde_json::json!({
                "prompt": prompt,
                "n": args.n.unwrap_or(1),
                "size": args.size.clone().unwrap_or_else(|| "2K".into()),
                "enable_sequential": args.enable_sequential,
                "negative_prompt": args.negative_prompt.clone().unwrap_or_default(),
            });
            Ok((TaskKind::Image, v.to_string()))
        }
        "happyhorse-1.0-r2v" => {
            if args.subject.is_empty() {
                bail!("--subject is required for happyhorse-1.0-r2v");
            }
            let v = serde_json::json!({
                "prompt": prompt,
                "subjects": args.subject,
                "resolution": args.resolution.clone().unwrap_or_else(|| "720P".into()),
                "ratio": args.ratio.clone().unwrap_or_else(|| "16:9".into()),
                "duration": args.duration.unwrap_or(5),
            });
            Ok((TaskKind::Video, v.to_string()))
        }
        other => bail!(
            "unknown model id '{}'. Supported: wan2.7-image-pro, happyhorse-1.0-r2v",
            other
        ),
    }
}

async fn setup_engine(db: AsyncConnection) -> anyhow::Result<TaskEngineHandle> {
    let keyring: Arc<dyn KeyringStore> = Arc::new(SystemKeyring);
    let providers = ProviderRegistry::builder()
        .register("bailian", Arc::new(BailianProvider::new()))
        .build();
    let materializer: Arc<dyn mango_core::task_engine::ResultMaterializer> =
        Arc::new(BailianResultMaterializer::new(db.clone(), keyring.clone()));

    let (engine, _events_rx) =
        TaskEngineHandle::spawn(db, providers, keyring, materializer, 4);

    Ok(engine)
}

async fn wait_for_terminal(
    engine: &TaskEngineHandle,
    task_id: &str,
    json: bool,
) -> anyhow::Result<i32> {
    let started = Instant::now();
    let mut last_status: Option<GenerationTaskStatus> = None;

    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;

        let task = engine
            .get(task_id)
            .await
            .map_err(|e| anyhow::anyhow!("failed to get task status: {e}"))?;

        if last_status != Some(task.status) {
            if !json {
                eprintln!(
                    "{} {}",
                    output::status_symbol(task.status),
                    output::status_display(task.status)
                );
            }
            last_status = Some(task.status);
        }

        if task.status.is_terminal() {
            return finish_wait(&task, started.elapsed(), json);
        }
    }
}

fn finish_wait(
    task: &GenerationTask,
    elapsed: Duration,
    json: bool,
) -> anyhow::Result<i32> {
    if json {
        let out = serde_json::json!({
            "task_id": task.id,
            "status": output::status_display(task.status),
            "duration_ms": elapsed.as_millis(),
            "result_asset_id": task.result_asset_id,
            "model_id": task.model_id,
            "task_type": output::kind_display(task.task_type),
        });
        println!("{}", serde_json::to_string(&out)?);
    } else {
        match task.status {
            GenerationTaskStatus::Success => {
                eprintln!(
                    "✓ success in {:.1}s",
                    elapsed.as_secs_f64()
                );
                // stdout: task_id and result
                println!("{}", task.id);
                if let Some(ref asset_id) = task.result_asset_id {
                    println!("  result_asset: {asset_id}");
                }
            }
            GenerationTaskStatus::Failed => {
                eprintln!(
                    "✗ failed in {:.1}s: {}",
                    elapsed.as_secs_f64(),
                    task.error_message.as_deref().unwrap_or("unknown error")
                );
            }
            GenerationTaskStatus::Cancelled => {
                eprintln!("⊘ cancelled");
            }
            _ => {}
        }
    }

    Ok(match task.status {
        GenerationTaskStatus::Success => 0,
        GenerationTaskStatus::Failed => 10,
        GenerationTaskStatus::Cancelled => 11,
        _ => 1,
    })
}
