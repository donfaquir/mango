//! HTTP client wrapper for DashScope REST API.
//!
//! Provides typed request methods (`post_json`, `post_json_async`, `get_json`,
//! `delete`) and maps HTTP status codes to [`ProviderErrorKind`]. No retry
//! logic — errors are surfaced directly with `request_id` and `http_status`
//! attached so the diagnostics UI can render them.
//!
//! Every fallible method returns `(R, ResponseMeta)` on success, so callers
//! can record an event with the upstream `x-dashscope-request-id` even on
//! the happy path.

use serde::{de::DeserializeOwned, Serialize};

use crate::error::{CoreError, Result};
use crate::provider::error::{ProviderErrorDetail, ProviderErrorKind};
use crate::provider::traits::ProviderCredentials;

const DEFAULT_BASE_URL: &str = "https://dashscope.aliyuncs.com/api/v1";

/// Metadata captured from a DashScope HTTP response. Surfaced to the caller
/// (success path) and embedded inside `ProviderErrorDetail` (failure path) so
/// the event log can attribute every roundtrip to a specific upstream call.
#[derive(Debug, Clone)]
pub(super) struct ResponseMeta {
    pub request_id: Option<String>,
    pub http_status: u16,
}

pub(super) struct BailianClient<'a> {
    http: &'a reqwest::Client,
    base_url: String,
    api_key: String,
}

impl<'a> BailianClient<'a> {
    pub fn new(http: &'a reqwest::Client, creds: &ProviderCredentials) -> Result<Self> {
        if creds.api_key.is_empty() {
            return Err(CoreError::Provider(ProviderErrorDetail::new(
                ProviderErrorKind::Auth,
                "百炼 API Key 为空",
            )));
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
    ) -> Result<(R, ResponseMeta)> {
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
    ) -> Result<(R, ResponseMeta)> {
        self.send(
            self.http
                .post(format!("{}{path}", self.base_url))
                .json(body)
                .header("X-DashScope-Async", "enable"),
        )
        .await
    }

    pub async fn get_json<R: DeserializeOwned>(&self, path: &str) -> Result<(R, ResponseMeta)> {
        self.send(self.http.get(format!("{}{path}", self.base_url)))
            .await
    }

    /// Best-effort DELETE used by cancel paths. Returns `ResponseMeta` so
    /// callers may still log the upstream `request_id`, but always succeeds.
    pub async fn delete(&self, path: &str) -> Result<ResponseMeta> {
        let resp = self
            .http
            .delete(format!("{}{path}", self.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| CoreError::Provider(network_or_timeout_detail(&e)))?;
        let status = resp.status().as_u16();
        let request_id = extract_request_id_from_headers(resp.headers());
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            tracing::debug!("delete {path} returned {status}: {body}");
        }
        Ok(ResponseMeta {
            request_id,
            http_status: status,
        })
    }

    async fn send<R: DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
    ) -> Result<(R, ResponseMeta)> {
        let resp = req
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| CoreError::Provider(network_or_timeout_detail(&e)))?;

        let status = resp.status().as_u16();
        let header_request_id = extract_request_id_from_headers(resp.headers());
        let body = resp.text().await.unwrap_or_default();
        // Trim body to 1KB for error messages to avoid leaking large responses.
        let body_trimmed = if body.len() > 1024 {
            format!("{}...(truncated)", &body[..1024])
        } else {
            body.clone()
        };
        let request_id = header_request_id
            .clone()
            .or_else(|| extract_request_id_from_body(&body));
        let meta = ResponseMeta {
            request_id: request_id.clone(),
            http_status: status,
        };

        match status {
            200..=299 => match serde_json::from_str::<R>(&body) {
                Ok(parsed) => Ok((parsed, meta)),
                Err(e) => Err(CoreError::Provider(
                    ProviderErrorDetail::new(
                        ProviderErrorKind::Malformed,
                        format!("响应解析失败: {e}"),
                    )
                    .with_http_status(status)
                    .with_request_id(request_id)
                    .with_body_excerpt(body_trimmed),
                )),
            },
            401 => Err(CoreError::Provider(
                ProviderErrorDetail::new(ProviderErrorKind::Auth, "认证失败：API Key 无效或已过期")
                    .with_http_status(status)
                    .with_request_id(request_id)
                    .with_body_excerpt(body_trimmed),
            )),
            403 => {
                let kind = if body.contains("InsufficientBalance") || body.contains("Quota") {
                    ProviderErrorKind::Quota
                } else {
                    ProviderErrorKind::Auth
                };
                let msg = match kind {
                    ProviderErrorKind::Quota => "余额不足或配额已用尽",
                    _ => "认证失败：权限不足",
                };
                Err(CoreError::Provider(
                    ProviderErrorDetail::new(kind, msg)
                        .with_http_status(status)
                        .with_request_id(request_id)
                        .with_body_excerpt(body_trimmed),
                ))
            }
            429 => Err(CoreError::Provider(
                ProviderErrorDetail::new(ProviderErrorKind::RateLimited, "请求过于频繁，已限流")
                    .with_http_status(status)
                    .with_request_id(request_id)
                    .with_body_excerpt(body_trimmed),
            )),
            400..=499 => Err(CoreError::Provider(
                ProviderErrorDetail::new(
                    ProviderErrorKind::InvalidRequest,
                    format!("请求被拒绝 (HTTP {status})"),
                )
                .with_http_status(status)
                .with_request_id(request_id)
                .with_body_excerpt(body_trimmed),
            )),
            _ => Err(CoreError::Provider(
                ProviderErrorDetail::new(
                    ProviderErrorKind::Unknown,
                    format!("远端服务异常 (HTTP {status})"),
                )
                .with_http_status(status)
                .with_request_id(request_id)
                .with_body_excerpt(body_trimmed),
            )),
        }
    }
}

fn extract_request_id_from_headers(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get("x-dashscope-request-id")
        .and_then(|v| v.to_str().ok())
        .map(String::from)
}

fn extract_request_id_from_body(body: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| {
            v.get("request_id")
                .and_then(|x| x.as_str())
                .map(String::from)
        })
}

fn network_or_timeout_detail(e: &reqwest::Error) -> ProviderErrorDetail {
    if e.is_timeout() {
        ProviderErrorDetail::new(ProviderErrorKind::Timeout, "请求超时")
    } else {
        ProviderErrorDetail::new(ProviderErrorKind::Network, format!("网络错误: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_request_id_from_body_picks_top_level_field() {
        let body = r#"{"request_id":"rid-1","output":{}}"#;
        assert_eq!(extract_request_id_from_body(body), Some("rid-1".into()));
    }

    #[test]
    fn extract_request_id_from_body_returns_none_for_garbage() {
        assert_eq!(extract_request_id_from_body("not json"), None);
        assert_eq!(extract_request_id_from_body("{}"), None);
    }
}
