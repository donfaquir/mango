//! In-memory async task engine. Drives `generation_task` rows through the
//! `pending → running → terminal` state machine using a registered
//! [`crate::provider::ModelProvider`]. Engine state lives in [`TaskEngineHandle`];
//! the runner coroutines are spawned per-submission and exit on terminal state
//! or cancellation.

pub mod events;
pub mod handle;
pub mod materializer;
pub mod runner;
pub mod state;

pub use events::TaskEvent;
pub use handle::{ListFilter, SubmitBatchOutcome, TaskEngineHandle};
pub use materializer::{CleanupOutcome, MaterializeOutcome, NoopMaterializer, ResultMaterializer};
pub use state::EventBuilder;
