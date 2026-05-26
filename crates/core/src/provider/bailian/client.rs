//! HTTP client wrapper for DashScope REST API.
//!
//! Provides typed request methods (`post_json`, `post_json_async`, `get_json`,
//! `delete`) and maps HTTP status codes to [`BailianError`]. No retry logic —
//! errors are surfaced directly and the UI offers "retry" to the user.

use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

use crate::error::{CoreError, Result};
use crate::provider::traits::ProviderCredentials;

const DEFAULT_BASE_URL: &str = "https://dashscope.aliyuncs.com/api/v1";

/// Private error enum for fine-grained HTTP/API error classification.
/// Collapsed to `CoreError::Provider(String)` at the crate boundary.
#[derive(Error, Debug)]
pub(super) enum BailianError {
    #[error("authentication failed: invalid or expired API key")]
    AuthFailed,
    #[error("rate limited (HTTP 429)")]
    RateLimited,
    #[error("insufficient quota or balance")]
    QuotaExhausted,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("remote service error (HTTP {status}): {body}")]
    RemoteError { status: u16, body: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("timeout after {secs}s")]
    Timeout { secs: u32 },
    #[error("malformed response: {0}")]
    MalformedResponse(String),
}

impl From<BailianError> for CoreError {
    fn from(e: BailianError) -> Self {
        CoreError::Provider(e.to_string())
    }
}

pub(super) struct BailianClient<'a> {
    http: &'a reqwest::Client,
    base_url: String,
    api_key: String,
}

impl<'a> BailianClient<'a> {
    pub fn new(http: &'a reqwest::Client, creds: &ProviderCredentials) -> Result<Self> {
        if creds.api_key.is_empty() {
            return Err(CoreError::Provider("bailian api_key empty".into()));
        }
        Ok(Self {
            http,
            base_url: DEFAULT_BASE_URL.into(),
            api_key: creds.api_key.clone(),
        })
    }

    /// For testing: allow overriding the base URL.
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn with_base_url(mut self, url: String) -> Self {
        self.base_url = url;
        self
    }

    /// Synchronous-style POST (no X-DashScope-Async header).
    pub async fn post_json<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R> {
        self.send(
            self.http
                .post(format!("{}{path}", self.base_url))
                .json(body),
        )
        .await
    }

    /// Async-style POST (with X-DashScope-Async: enable header).
    pub async fn post_json_async<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<R> {
        self.send(
            self.http
                .post(format!("{}{path}", self.base_url))
                .json(body)
                .header("X-DashScope-Async", "enable"),
        )
        .await
    }

    pub async fn get_json<R: DeserializeOwned>(&self, path: &str) -> Result<R> {
        self.send(self.http.get(format!("{}{path}", self.base_url)))
            .await
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let resp = self
            .http
            .delete(format!("{}{path}", self.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    BailianError::Timeout { secs: 60 }
                } else {
                    BailianError::Network(e.to_string())
                }
            })?;
        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            tracing::debug!("delete {path} returned {status}: {body}");
        }
        // Best-effort: always return Ok for cancel operations.
        Ok(())
    }

    async fn send<R: DeserializeOwned>(&self, req: reqwest::RequestBuilder) -> Result<R> {
        let resp = req.bearer_auth(&self.api_key).send().await.map_err(|e| {
            if e.is_timeout() {
                BailianError::Timeout { secs: 60 }
            } else {
                BailianError::Network(e.to_string())
            }
        })?;

        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        // Trim body to 1KB for error messages to avoid leaking large responses.
        let body_trimmed = if body.len() > 1024 {
            format!("{}...(truncated)", &body[..1024])
        } else {
            body.clone()
        };

        match status {
            200..=299 => serde_json::from_str(&body).map_err(|e| {
                BailianError::MalformedResponse(format!("{e}; body: {body_trimmed}")).into()
            }),
            401 => Err(BailianError::AuthFailed.into()),
            403 => {
                if body.contains("InsufficientBalance") || body.contains("Quota") {
                    Err(BailianError::QuotaExhausted.into())
                } else {
                    Err(BailianError::AuthFailed.into())
                }
            }
            429 => Err(BailianError::RateLimited.into()),
            400..=499 => Err(BailianError::InvalidRequest(body_trimmed).into()),
            _ => Err(BailianError::RemoteError {
                status,
                body: body_trimmed,
            }
            .into()),
        }
    }
}
