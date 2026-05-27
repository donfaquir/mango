//! Internal mpsc event type. The Tauri shell forwards these to the front-end
//! via `tauri-specta` events; the CLI can drop the receiver (or, in
//! `--wait` mode, attach a printer).
//!
//! Three variants:
//! - `StatusChanged` — the canonical state-machine transition; fired whenever
//!   `state::transition` writes a new status row.
//! - `EventLogged` — a freshly inserted `generation_task_event` row, surfaced
//!   in real time so the diagnostics UI/CLI doesn't have to poll the DB.
//! - `ProgressTick` — purely ephemeral progress hint; not persisted. Used when
//!   the provider reports a numeric percentage during `Running`.

use crate::models::generation_task::GenerationTaskStatus;
use crate::models::generation_task_event::GenerationTaskEvent;

#[derive(Debug, Clone)]
pub enum TaskEvent {
    StatusChanged {
        task_id: String,
        status: GenerationTaskStatus,
        progress: Option<u8>,
        error_message: Option<String>,
    },
    EventLogged {
        task_id: String,
        event: GenerationTaskEvent,
    },
    ProgressTick {
        task_id: String,
        progress: u8,
    },
}

impl TaskEvent {
    pub fn status_changed(
        task_id: impl Into<String>,
        status: GenerationTaskStatus,
        progress: Option<u8>,
        error_message: Option<String>,
    ) -> Self {
        Self::StatusChanged {
            task_id: task_id.into(),
            status,
            progress,
            error_message,
        }
    }

    pub fn event_logged(task_id: impl Into<String>, event: GenerationTaskEvent) -> Self {
        Self::EventLogged {
            task_id: task_id.into(),
            event,
        }
    }

    pub fn progress_tick(task_id: impl Into<String>, progress: u8) -> Self {
        Self::ProgressTick {
            task_id: task_id.into(),
            progress,
        }
    }
}
