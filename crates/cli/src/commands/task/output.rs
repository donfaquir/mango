//! Shared output helpers: JSON serialization, table formatting, status symbols.

use mango_core::models::generation_task::{GenerationTask, GenerationTaskStatus, TaskKind};
use mango_core::models::generation_task_event::{
    EventPhase, EventSeverity, GenerationTaskEvent,
};

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

fn phase_label(phase: EventPhase) -> &'static str {
    match phase {
        EventPhase::SubmitUpload => "submit_upload",
        EventPhase::SubmitCall => "submit_call",
        EventPhase::Poll => "poll",
        EventPhase::Download => "download",
        EventPhase::Persist => "persist",
        EventPhase::Cleanup => "cleanup",
    }
}

fn severity_label(severity: EventSeverity) -> &'static str {
    match severity {
        EventSeverity::Info => "info",
        EventSeverity::Warn => "warn",
        EventSeverity::Error => "error",
    }
}

/// Render a single event as a human-readable line. Stable format used by
/// both `task status --verbose` and the `task submit --wait` real-time
/// printer so users see consistent output.
pub fn format_event_line(event: &GenerationTaskEvent) -> String {
    let time = event.occurred_at.split(' ').nth(1).unwrap_or(&event.occurred_at);
    let mut suffix = String::new();
    if let Some(s) = event.http_status {
        suffix.push_str(&format!(" HTTP {s}"));
    }
    if let Some(rid) = &event.request_id {
        suffix.push_str(&format!(" request_id={rid}"));
    }
    format!(
        "[{time}] {phase:<14} {severity:<6} {message}{suffix}",
        phase = phase_label(event.phase),
        severity = severity_label(event.severity),
        message = event.message,
        suffix = suffix,
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(
        phase: EventPhase,
        severity: EventSeverity,
        message: &str,
        request_id: Option<&str>,
        http_status: Option<i64>,
    ) -> GenerationTaskEvent {
        GenerationTaskEvent {
            id: 1,
            task_id: "t".into(),
            occurred_at: "2026-05-26 10:00:01".into(),
            phase,
            severity,
            request_id: request_id.map(String::from),
            http_status,
            details_json: "{}".into(),
            message: message.into(),
        }
    }

    #[test]
    fn format_event_line_renders_time_phase_severity_and_message() {
        let line = format_event_line(&ev(
            EventPhase::SubmitCall,
            EventSeverity::Info,
            "提交到云端",
            None,
            None,
        ));
        assert_eq!(line, "[10:00:01] submit_call    info   提交到云端");
    }

    #[test]
    fn format_event_line_appends_http_and_request_id_when_present() {
        let line = format_event_line(&ev(
            EventPhase::Poll,
            EventSeverity::Error,
            "认证失败",
            Some("rid-xyz"),
            Some(401),
        ));
        assert!(line.contains("poll"));
        assert!(line.contains("error"));
        assert!(line.ends_with("认证失败 HTTP 401 request_id=rid-xyz"));
    }

    #[test]
    fn format_event_line_falls_back_to_full_string_when_no_space() {
        // Some clients may emit ISO timestamps without a space separator. The
        // fallback should at least render the message instead of stripping it.
        let mut e = ev(
            EventPhase::Cleanup,
            EventSeverity::Warn,
            "清理失败",
            None,
            None,
        );
        e.occurred_at = "2026-05-26T10:00:01Z".into();
        let line = format_event_line(&e);
        assert!(line.starts_with("[2026-05-26T10:00:01Z]"));
    }
}
