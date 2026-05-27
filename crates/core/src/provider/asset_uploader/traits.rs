//! `AssetUploader` trait + value types.
//!
//! Defines the contract used by spec-17's `BailianProvider` (and any future
//! provider that requires a public HTTPS URL for inputs) to push local files
//! to a publicly-reachable URL and clean them up after the job terminates.
//! The trait is provider-agnostic: today only [`super::oss::OssUploader`]
//! implements it, but S3/COS/etc. fit the same shape.

use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::Result;

/// Result of a successful [`AssetUploader::upload`]. Hands the caller back:
/// 1. a public HTTPS `url` it can pass to the model;
/// 2. an opaque `remote_id` used solely to drive `cleanup`;
/// 3. an `expires_at` timestamp so callers can refuse to forward stale URLs;
/// 4. `bytes` + `duration_ms` so the diagnostics event log can show how long
///    each upload took and how much it pushed.
///
/// Implementations choose what `remote_id` is (OSS object key today). The
/// trait stays neutral so swapping backends doesn't leak through the value.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UploadedAsset {
    /// Public HTTPS URL — typically presigned with a short expiry. Anything
    /// the destination model needs to GET goes here.
    pub url: String,
    /// Backend handle used by `cleanup`. Not meant for the model.
    pub remote_id: String,
    /// ISO-8601 UTC timestamp at which `url` stops being usable.
    pub expires_at: String,
    /// Size of the uploaded payload, in bytes.
    #[specta(type = specta_typescript::Number)]
    pub bytes: u64,
    /// Wall-clock time the upload took, in milliseconds.
    #[specta(type = specta_typescript::Number)]
    pub duration_ms: u64,
}

#[async_trait]
pub trait AssetUploader: Send + Sync {
    /// Push a local file to the backend and return a signed URL.
    async fn upload(&self, local_path: &Path) -> Result<UploadedAsset>;

    /// Best-effort delete. Failures must not poison the surrounding task —
    /// implementations log and return `Ok(())` rather than bubbling network
    /// errors that would force the engine to retry.
    async fn cleanup(&self, remote_id: &str) -> Result<()>;
}
