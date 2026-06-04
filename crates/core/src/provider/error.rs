//! Structured error type for provider/upload failures.
//!
//! `ProviderErrorDetail` carries the four pieces of information a debugging
//! user needs after a task fails: classification (`kind`), upstream HTTP
//! `http_status`, the upstream `request_id` (the only thing operators can
//! quote when filing tickets with the model vendor), and a human-readable
//! Chinese `message` for direct UI display.
//!
//! `body_excerpt` is opt-in and capped at 1 KiB upstream so we can include
//! the upstream JSON snippet in diagnostics without leaking large bodies.

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    /// 401 / 403 — invalid or expired API key, missing scope.
    Auth,
    /// 429 — over rate limit; retryable with backoff.
    RateLimited,
    /// 403 with quota/balance signal — non-retryable until topped up.
    Quota,
    /// 4xx — schema or business validation failure.
    InvalidRequest,
    /// Connection refused, DNS, TLS — typically transient.
    Network,
    /// Client-side timeout.
    Timeout,
    /// 2xx with body that does not deserialize.
    Malformed,
    /// 5xx or anything we did not classify.
    Unknown,
}

impl ProviderErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "auth",
            Self::RateLimited => "rate_limited",
            Self::Quota => "quota",
            Self::InvalidRequest => "invalid_request",
            Self::Network => "network",
            Self::Timeout => "timeout",
            Self::Malformed => "malformed",
            Self::Unknown => "unknown",
        }
    }

    /// True when the failure looks transient enough that a backoff retry is
    /// worth attempting. spec-27 §3.1. `Unknown` is NOT considered retryable
    /// here — the runner separately upgrades `Unknown` with HTTP 5xx because
    /// that information lives on the detail, not the kind.
    pub fn is_retryable(self) -> bool {
        matches!(self, Self::RateLimited | Self::Network | Self::Timeout)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ProviderErrorDetail {
    pub kind: ProviderErrorKind,
    #[specta(type = Option<specta_typescript::Number>)]
    pub http_status: Option<i64>,
    pub request_id: Option<String>,
    pub body_excerpt: Option<String>,
    /// Human-readable Chinese message; rendered directly in the UI.
    pub message: String,
}

impl ProviderErrorDetail {
    pub fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            http_status: None,
            request_id: None,
            body_excerpt: None,
            message: message.into(),
        }
    }

    pub fn with_http_status(mut self, status: u16) -> Self {
        self.http_status = Some(status as i64);
        self
    }

    pub fn with_request_id(mut self, id: Option<String>) -> Self {
        self.request_id = id;
        self
    }

    pub fn with_body_excerpt(mut self, body: impl Into<String>) -> Self {
        self.body_excerpt = Some(body.into());
        self
    }
}

impl std::fmt::Display for ProviderErrorDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(status) = self.http_status {
            write!(f, " (HTTP {status})")?;
        }
        if let Some(rid) = &self.request_id {
            write!(f, " [request_id={rid}]")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_status_and_request_id_when_present() {
        let d = ProviderErrorDetail::new(ProviderErrorKind::Auth, "认证失败")
            .with_http_status(401)
            .with_request_id(Some("rid-1".into()));
        assert_eq!(d.to_string(), "认证失败 (HTTP 401) [request_id=rid-1]");
    }

    #[test]
    fn display_omits_optional_fields() {
        let d = ProviderErrorDetail::new(ProviderErrorKind::Network, "连接失败");
        assert_eq!(d.to_string(), "连接失败");
    }

    #[test]
    fn is_retryable_matches_expected_kinds() {
        assert!(ProviderErrorKind::RateLimited.is_retryable());
        assert!(ProviderErrorKind::Network.is_retryable());
        assert!(ProviderErrorKind::Timeout.is_retryable());
        // Non-retryable: authoritative client errors should not be retried.
        assert!(!ProviderErrorKind::Auth.is_retryable());
        assert!(!ProviderErrorKind::Quota.is_retryable());
        assert!(!ProviderErrorKind::InvalidRequest.is_retryable());
        assert!(!ProviderErrorKind::Malformed.is_retryable());
        // Unknown is handled separately by the runner via http_status.
        assert!(!ProviderErrorKind::Unknown.is_retryable());
    }

    #[test]
    fn kind_as_str_round_trip() {
        let cases = [
            (ProviderErrorKind::Auth, "auth"),
            (ProviderErrorKind::RateLimited, "rate_limited"),
            (ProviderErrorKind::Quota, "quota"),
            (ProviderErrorKind::InvalidRequest, "invalid_request"),
            (ProviderErrorKind::Network, "network"),
            (ProviderErrorKind::Timeout, "timeout"),
            (ProviderErrorKind::Malformed, "malformed"),
            (ProviderErrorKind::Unknown, "unknown"),
        ];
        for (k, s) in cases {
            assert_eq!(k.as_str(), s);
        }
    }
}
