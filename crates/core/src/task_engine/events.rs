//! Internal mpsc event type. The Tauri shell forwards these to the front-end
//! via `tauri-specta` events; the CLI can drop the receiver.

use crate::models::generation_task::GenerationTaskStatus;

#[derive(Debug, Clone)]
pub enum TaskEvent {
    StatusChanged {
        task_id: String,
        status: GenerationTaskStatus,
        progress: Option<u8>,
        error_message: Option<String>,
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
}
