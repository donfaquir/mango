//! Shared output helpers: JSON serialization, table formatting, status symbols.

use mango_core::models::generation_task::{GenerationTask, GenerationTaskStatus, TaskKind};

/// Short 8-char ID for table display.
pub fn short_id(id: &str) -> String {
    let n = 8.min(id.len());
    id[..n].to_string()
}

/// Human-readable status symbol + text.
pub fn status_display(status: GenerationTaskStatus) -> &'static str {
    match status {
        GenerationTaskStatus::Pending => "pending",
        GenerationTaskStatus::Running => "running",
        GenerationTaskStatus::Success => "success",
        GenerationTaskStatus::Failed => "failed",
        GenerationTaskStatus::Cancelled => "cancelled",
    }
}

/// Status prefix symbol for progress lines (stderr).
pub fn status_symbol(status: GenerationTaskStatus) -> &'static str {
    match status {
        GenerationTaskStatus::Pending => "⏳",
        GenerationTaskStatus::Running => "↻",
        GenerationTaskStatus::Success => "✓",
        GenerationTaskStatus::Failed => "✗",
        GenerationTaskStatus::Cancelled => "⊘",
    }
}

/// Task kind display string.
pub fn kind_display(kind: TaskKind) -> &'static str {
    match kind {
        TaskKind::Text => "text",
        TaskKind::Image => "image",
        TaskKind::Video => "video",
        TaskKind::Audio => "audio",
    }
}

/// Print a task as JSON to stdout (single line).
pub fn print_task_json(task: &GenerationTask) {
    if let Ok(json) = serde_json::to_string(task) {
        println!("{json}");
    }
}

/// Print a concise submitted summary.
pub fn print_submitted(task: &GenerationTask) {
    println!("Submitted");
    println!("  ID:        {}", task.id);
    println!("  Type:      {}", kind_display(task.task_type));
    println!("  Status:    {}", status_display(task.status));
    println!(
        "  Provider:  {} / {}",
        task.provider_id, task.model_id
    );
    println!(
        "  Account:   ****{}",
        &task.account_id[..8.min(task.account_id.len())]
    );
    println!();
    println!(
        "Run `mango task status {} --watch` to follow progress.",
        short_id(&task.id)
    );
}

/// Print a detailed task status block.
pub fn print_task_detail(task: &GenerationTask) {
    println!("Task {}", task.id);
    println!("  Status:        {}", status_display(task.status));
    println!("  Type:          {}", kind_display(task.task_type));
    println!(
        "  Provider:      {} / {}",
        task.provider_id, task.model_id
    );
    println!("  Account:       {}", short_id(&task.account_id));
    println!("  Submitted:     {}", task.created_at);
    if let Some(ref s) = task.started_at {
        println!("  Started:       {s}");
    }
    if let Some(ref f) = task.finished_at {
        println!("  Finished:      {f}");
    }
    if let Some(ref e) = task.error_message {
        println!("  Error:         {e}");
    }
    if let Some(ref a) = task.result_asset_id {
        println!("  Result asset:  {a}");
    }
}
