//! Tauri events emitted to the frontend. The forwarder coroutine in `setup`
//! drains the engine's mpsc receiver and re-emits each `TaskEvent` as one of
//! these typed `tauri-specta` events so the front-end gets a strongly typed
//! `events.taskStatusChanged.listen(...)` API.

use mango_core::models::generation_task::GenerationTaskStatus;
use mango_core::models::generation_task_event::GenerationTaskEvent;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

#[derive(Clone, Debug, Serialize, Deserialize, Type, Event)]
pub struct TaskStatusChanged {
    pub task_id: String,
    pub status: GenerationTaskStatus,
    /// 0..=100; `None` means the provider does not report progress.
    #[specta(type = Option<specta_typescript::Number>)]
    pub progress: Option<u8>,
    pub error_message: Option<String>,
}

/// A new diagnostic event has been persisted for `task_id`. Carries the full
/// row so the front-end can append to the cached timeline without an extra
/// `list_task_events` round-trip.
#[derive(Clone, Debug, Serialize, Deserialize, Type, Event)]
pub struct TaskEventLogged {
    pub task_id: String,
    pub event: GenerationTaskEvent,
}

/// Numeric progress hint emitted in real time during `Running`. Not persisted —
/// purely a UI heartbeat. The front-end should display this only while the
/// task is in `Running`; cleared on terminal status.
#[derive(Clone, Debug, Serialize, Deserialize, Type, Event)]
pub struct TaskProgressTick {
    pub task_id: String,
    #[specta(type = specta_typescript::Number)]
    pub progress: u8,
}
