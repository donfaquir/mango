//! Tauri events emitted to the frontend. The forwarder coroutine in `setup`
//! drains the engine's mpsc receiver and re-emits each `TaskEvent` as one of
//! these typed `tauri-specta` events so the front-end gets a strongly typed
//! `events.taskStatusChanged.listen(...)` API.

use mango_core::models::generation_task::GenerationTaskStatus;
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
