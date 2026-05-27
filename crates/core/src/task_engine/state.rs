//! Thin async wrapper layer between the runner and the DB. Each helper:
//!   1. Schedules a synchronous DB closure on the tokio-rusqlite worker thread.
//!   2. On success, emits a [`TaskEvent`] when the change is observable to the UI.
//!
//! Keeping the SQL inside `db::queries::generation_task` (single source of truth)
//! and putting only the cross-cutting concerns (events) here.

use tokio::sync::mpsc::UnboundedSender;
use tokio_rusqlite::Connection as AsyncConnection;

use crate::db::queries::generation_task as q;
use crate::db::queries::generation_task_event as event_q;
use crate::error::{CoreError, Result};
use crate::models::generation_task::{GenerationTask, GenerationTaskStatus};
use crate::models::generation_task_event::{EventPhase, EventSeverity, GenerationTaskEvent};
use crate::provider::error::ProviderErrorDetail;

use super::events::TaskEvent;

pub async fn load(db: &AsyncConnection, task_id: &str) -> Result<GenerationTask> {
    let id = task_id.to_string();
    db.call(move |conn| Ok(q::get_by_id(conn, &id)))
        .await
        .map_err(map_async_err)?
}

pub async fn is_cancelled(db: &AsyncConnection, task_id: &str) -> Result<bool> {
    let task = load(db, task_id).await?;
    Ok(matches!(task.status, GenerationTaskStatus::Cancelled))
}

pub async fn set_external_id(db: &AsyncConnection, task_id: &str, external_id: &str) -> Result<()> {
    let id = task_id.to_string();
    let ext = external_id.to_string();
    db.call(move |conn| Ok(q::set_external_id(conn, &id, &ext)))
        .await
        .map_err(map_async_err)?
}

pub async fn set_result_asset_id(db: &AsyncConnection, task_id: &str, asset_id: &str) -> Result<()> {
    let id = task_id.to_string();
    let aid = asset_id.to_string();
    db.call(move |conn| Ok(q::set_result_asset_id(conn, &id, &aid)))
        .await
        .map_err(map_async_err)?
}

/// Apply a state-machine transition and emit a [`TaskEvent::StatusChanged`].
/// `progress` is informational only — the DB does not store it; it rides along
/// in the event so the UI can render a percentage during `Running`.
pub async fn transition(
    db: &AsyncConnection,
    event_tx: &UnboundedSender<TaskEvent>,
    task_id: &str,
    new_status: GenerationTaskStatus,
    progress: Option<u8>,
    error_message: Option<String>,
) -> Result<()> {
    let id = task_id.to_string();
    let err_clone = error_message.clone();
    db.call(move |conn| Ok(q::transition_status(conn, &id, new_status, err_clone.as_deref())))
        .await
        .map_err(map_async_err)??;

    // Best-effort send. A dropped receiver means the forwarder coroutine
    // exited; the engine still functions, the UI just stops getting events.
    let _ = event_tx.send(TaskEvent::status_changed(
        task_id,
        new_status,
        progress,
        error_message,
    ));
    Ok(())
}

fn map_async_err(e: tokio_rusqlite::Error) -> CoreError {
    match e {
        tokio_rusqlite::Error::Error(inner) => CoreError::Sqlite(inner),
        other => CoreError::TaskEngine(format!("db worker error: {other}")),
    }
}

/// Builder for the optional fields on a diagnostic event. Lets callers spell
/// out `request_id`/`http_status`/`details_json` only when they have them,
/// without making `record_event` a 7-positional-argument function.
#[derive(Debug, Default)]
pub struct EventBuilder {
    pub request_id: Option<String>,
    pub http_status: Option<i64>,
    pub details_json: Option<String>,
}

impl EventBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seed `request_id`/`http_status` from a [`ProviderErrorDetail`]. The
    /// caller still picks `phase`/`severity` and writes the `message` in
    /// caller-friendly Chinese — the detail's English `Display` is not used
    /// because the event matrix wants short, UI-ready phrases.
    pub fn from_detail(detail: &ProviderErrorDetail) -> Self {
        let mut details = serde_json::Map::new();
        details.insert(
            "kind".into(),
            serde_json::Value::String(detail.kind.as_str().to_string()),
        );
        if let Some(excerpt) = &detail.body_excerpt {
            details.insert(
                "body_excerpt".into(),
                serde_json::Value::String(redact(excerpt)),
            );
        }
        Self {
            request_id: detail.request_id.clone(),
            http_status: detail.http_status,
            details_json: Some(
                serde_json::to_string(&serde_json::Value::Object(details)).unwrap_or_else(|_| "{}".into()),
            ),
        }
    }

    pub fn request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    pub fn maybe_request_id(mut self, request_id: Option<String>) -> Self {
        self.request_id = request_id;
        self
    }

    pub fn http_status(mut self, http_status: i64) -> Self {
        self.http_status = Some(http_status);
        self
    }

    pub fn maybe_http_status(mut self, http_status: Option<i64>) -> Self {
        self.http_status = http_status;
        self
    }

    /// Set the `details_json` payload from any `Serialize` value. Keys that
    /// look like secrets are redacted; any value-string above 1KB is
    /// truncated. Failures in serialization are downgraded to `{}` rather
    /// than bubbling — the diagnostics layer must never break the runner.
    pub fn details<T: serde::Serialize>(mut self, value: T) -> Self {
        match serde_json::to_value(value) {
            Ok(mut v) => {
                redact_in_place(&mut v);
                self.details_json = Some(v.to_string());
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to serialize event details");
                self.details_json = Some("{}".into());
            }
        }
        self
    }
}

const SECRET_KEY_NEEDLES: &[&str] = &[
    "secret",
    "api_key",
    "access_key",
    "authorization",
    "password",
    "token",
];
const MAX_DETAIL_VALUE_LEN: usize = 1024;
const REDACTED: &str = "***";

fn redact_in_place(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                if is_secret_key(k) {
                    *v = serde_json::Value::String(REDACTED.into());
                } else {
                    redact_in_place(v);
                }
            }
        }
        serde_json::Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_in_place(item);
            }
        }
        serde_json::Value::String(s) => {
            if s.len() > MAX_DETAIL_VALUE_LEN {
                *s = redact(s);
            }
        }
        _ => {}
    }
}

fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SECRET_KEY_NEEDLES.iter().any(|n| lower.contains(n))
}

fn redact(s: &str) -> String {
    if s.len() <= MAX_DETAIL_VALUE_LEN {
        s.to_string()
    } else {
        let mut out = s[..MAX_DETAIL_VALUE_LEN].to_string();
        out.push_str("…(truncated)");
        out
    }
}

/// Insert a `generation_task_event` row and emit a [`TaskEvent::EventLogged`]
/// on the engine bus. Returns `Ok(None)` when the per-task cap is hit (the
/// row is silently dropped). Internal failures are downgraded to a
/// `tracing::warn!` so the diagnostics subsystem can never break the runner.
pub async fn record_event(
    db: &AsyncConnection,
    event_tx: &UnboundedSender<TaskEvent>,
    task_id: &str,
    phase: EventPhase,
    severity: EventSeverity,
    message: impl Into<String>,
    builder: EventBuilder,
) -> Option<GenerationTaskEvent> {
    let task_id_owned = task_id.to_string();
    let message_owned: String = message.into();
    let request_id_owned = builder.request_id;
    let http_status = builder.http_status;
    let details_owned = builder.details_json.unwrap_or_else(|| "{}".into());

    let task_id_for_call = task_id_owned.clone();
    let inserted: std::result::Result<Result<Option<GenerationTaskEvent>>, tokio_rusqlite::Error> =
        db.call(move |conn| {
            Ok(event_q::insert(
                conn,
                event_q::NewEvent {
                    task_id: &task_id_for_call,
                    phase,
                    severity,
                    request_id: request_id_owned.as_deref(),
                    http_status,
                    details_json: &details_owned,
                    message: &message_owned,
                },
            ))
        })
        .await;

    let row = match inserted {
        Ok(Ok(Some(row))) => row,
        Ok(Ok(None)) => return None,
        Ok(Err(e)) => {
            tracing::warn!(task_id = %task_id_owned, error = %e, "record_event insert failed");
            return None;
        }
        Err(e) => {
            tracing::warn!(task_id = %task_id_owned, error = %e, "record_event db worker failed");
            return None;
        }
    };

    let _ = event_tx.send(TaskEvent::event_logged(task_id_owned, row.clone()));
    Some(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_replaces_secret_keys() {
        let mut v = serde_json::json!({
            "remote_id": "abc",
            "access_key_id": "AKID-LEAK",
            "nested": { "Authorization": "Bearer xyz" }
        });
        redact_in_place(&mut v);
        assert_eq!(v["remote_id"], serde_json::json!("abc"));
        assert_eq!(v["access_key_id"], serde_json::json!(REDACTED));
        assert_eq!(v["nested"]["Authorization"], serde_json::json!(REDACTED));
    }

    #[test]
    fn redact_truncates_long_strings() {
        let mut s = String::with_capacity(MAX_DETAIL_VALUE_LEN + 10);
        for _ in 0..MAX_DETAIL_VALUE_LEN + 10 {
            s.push('a');
        }
        let mut v = serde_json::json!({ "msg": s });
        redact_in_place(&mut v);
        let truncated = v["msg"].as_str().unwrap();
        assert!(truncated.len() < MAX_DETAIL_VALUE_LEN + 20);
        assert!(truncated.ends_with("…(truncated)"));
    }

    #[test]
    fn event_builder_from_detail_captures_fields() {
        let detail = ProviderErrorDetail::new(
            crate::provider::error::ProviderErrorKind::RateLimited,
            "rate limited",
        )
        .with_http_status(429)
        .with_request_id(Some("req-9".into()))
        .with_body_excerpt("noise");
        let b = EventBuilder::from_detail(&detail);
        assert_eq!(b.http_status, Some(429));
        assert_eq!(b.request_id.as_deref(), Some("req-9"));
        let parsed: serde_json::Value =
            serde_json::from_str(b.details_json.as_deref().unwrap()).unwrap();
        assert_eq!(parsed["kind"], serde_json::json!("rate_limited"));
        assert_eq!(parsed["body_excerpt"], serde_json::json!("noise"));
    }
}
