//! Aliyun OSS implementation of [`AssetUploader`].
//!
//! Composition note: `aliyun-oss-client` 0.13.1 owns the upload + delete HTTP
//! plumbing (V1 header-signed PUT/DELETE), but does **not** expose a presigned
//! URL helper. We therefore hand-roll an OSS V1 query-signed GET URL using the
//! same HMAC-SHA1+base64 primitive the SDK uses internally. Choice of V1 over
//! V4 keeps the dependency surface tiny — V4 query signing requires several
//! hundred lines of canonical-string assembly and the OSS gateway accepts both.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use chrono::{Duration, Utc};
use hmac::{Hmac, Mac};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use sha1::Sha1;
use sha2::{Digest, Sha256};

use aliyun_oss_client::{Bucket, Client};

use super::traits::{AssetUploader, UploadedAsset};
use crate::error::{CoreError, Result};

type HmacSha1 = Hmac<Sha1>;

/// Default object-key prefix. Lets bucket operators distinguish mango-managed
/// objects from any other content sharing the bucket and supports operational
/// "delete everything under mango/refs/<old-date>/" cleanup.
const DEFAULT_PREFIX: &str = "mango/refs/";

/// Percent-encoding rule for OSS Signature query value: encode everything
/// except unreserved characters (RFC 3986 section 2.3). The OSS docs require
/// `+` `/` `=` to be percent-encoded inside `Signature=...`.
const SIGNATURE_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'+')
    .add(b'/')
    .add(b'=')
    .add(b'?')
    .add(b'#')
    .add(b'&');

/// `OssUploader` packages the four OSS coordinates (endpoint/bucket/AKID/AK
/// secret) plus a few operational knobs. The held `Bucket` and `Client` are
/// `Arc`'d by the SDK so cloning the uploader is cheap.
pub struct OssUploader {
    /// e.g. "oss-cn-hangzhou.aliyuncs.com"
    endpoint: String,
    bucket_name: String,
    access_key_id: String,
    access_key_secret: String,
    url_expires_seconds: u32,
    object_prefix: String,
    bucket: Bucket,
}

#[derive(Debug, serde::Deserialize)]
struct OssCredentialsJson {
    oss: OssBlock,
}

#[derive(Debug, serde::Deserialize)]
struct OssBlock {
    endpoint: String,
    bucket: String,
    access_key_id: String,
    access_key_secret: String,
    #[serde(default)]
    #[allow(dead_code)] // recorded for parity with public projection; not used
    region: Option<String>,
    #[serde(default = "default_expires")]
    url_expires_seconds: u32,
}

fn default_expires() -> u32 {
    3600
}

impl OssUploader {
    /// Build from the JSON injected by `account::service::resolve_credentials`
    /// into [`crate::provider::traits::ProviderCredentials::extra_json`].
    /// The expected shape is documented in SPEC-16:
    ///
    /// ```json
    /// { "oss": { "endpoint": "...", "bucket": "...", "access_key_id": "...",
    ///            "access_key_secret": "...", "region": "...",
    ///            "url_expires_seconds": 3600 } }
    /// ```
    pub fn from_credentials_json(json: &str) -> Result<Self> {
        let parsed: OssCredentialsJson = serde_json::from_str(json).map_err(|e| {
            CoreError::Validation(format!("oss credentials json: {e}"))
        })?;
        Self::new(
            parsed.oss.endpoint,
            parsed.oss.bucket,
            parsed.oss.access_key_id,
            parsed.oss.access_key_secret,
            parsed.oss.url_expires_seconds,
            None,
        )
    }

    /// Direct constructor — primarily for tests; production paths go through
    /// [`Self::from_credentials_json`].
    pub fn new(
        endpoint: String,
        bucket: String,
        access_key_id: String,
        access_key_secret: String,
        url_expires_seconds: u32,
        object_prefix: Option<String>,
    ) -> Result<Self> {
        let client = Client::new(
            access_key_id.clone(),
            access_key_secret.clone(),
            endpoint.as_str(),
        )
        .map_err(|e| CoreError::Validation(format!("oss endpoint: {e}")))?;
        let bucket_obj = Bucket::new(bucket.clone(), Arc::new(client))
            .map_err(|e| CoreError::Validation(format!("oss bucket: {e}")))?;

        Ok(Self {
            endpoint,
            bucket_name: bucket,
            access_key_id,
            access_key_secret,
            url_expires_seconds,
            object_prefix: object_prefix.unwrap_or_else(|| DEFAULT_PREFIX.to_string()),
            bucket: bucket_obj,
        })
    }

    /// Generate a V1 query-signed GET URL valid for `url_expires_seconds`.
    /// Object keys are expected to be ASCII URL-safe (we mint them ourselves
    /// from sha256 hex + date + extension), so we don't percent-encode the
    /// key path inside the URL itself — only the signature.
    pub fn presigned_get_url(&self, object_key: &str) -> Result<(String, chrono::DateTime<Utc>)> {
        let expires_at = Utc::now() + Duration::seconds(self.url_expires_seconds as i64);
        let expires_unix = expires_at.timestamp();

        // OSS V1 GET sign:
        //   StringToSign = "GET\n\n\n{Expires}\n/{bucket}/{object_key}"
        let string_to_sign = format!(
            "GET\n\n\n{expires_unix}\n/{bucket}/{key}",
            bucket = self.bucket_name,
            key = object_key,
        );

        let mut mac = HmacSha1::new_from_slice(self.access_key_secret.as_bytes())
            .map_err(|e| CoreError::Upload(format!("oss hmac key: {e}")))?;
        mac.update(string_to_sign.as_bytes());
        let signature = B64.encode(mac.finalize().into_bytes());
        let signature_enc =
            utf8_percent_encode(&signature, SIGNATURE_ENCODE_SET).to_string();
        let akid_enc =
            utf8_percent_encode(&self.access_key_id, SIGNATURE_ENCODE_SET).to_string();

        let url = format!(
            "https://{bucket}.{endpoint}/{key}?OSSAccessKeyId={akid}&Expires={exp}&Signature={sig}",
            bucket = self.bucket_name,
            endpoint = self.endpoint,
            key = object_key,
            akid = akid_enc,
            exp = expires_unix,
            sig = signature_enc,
        );
        Ok((url, expires_at))
    }

    /// Mint an object key from the file's sha256 + today's date + extension.
    /// Sha256 is for content-addressed deduplication (re-uploading the same
    /// reference image overwrites in place); the date prefix supports the
    /// "manually purge mango/refs/2026-05-* every week" operational habit.
    fn build_object_key(&self, bytes: &[u8], local_path: &Path) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hex(&hasher.finalize());
        let short = &digest[..16];

        let ext = local_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("bin");
        let date = Utc::now().format("%Y%m%d").to_string();
        format!("{}{}/{}.{}", self.object_prefix, date, short, ext)
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[async_trait]
impl AssetUploader for OssUploader {
    async fn upload(&self, local_path: &Path) -> Result<UploadedAsset> {
        let bytes = tokio::fs::read(local_path)
            .await
            .map_err(CoreError::Io)?;
        let object_key = self.build_object_key(&bytes, local_path);

        let object = self.bucket.object(&object_key);
        // The SDK returns its own `OssError`; collapse to our `Upload` variant
        // (string-shaped) rather than leaking SDK types through the IPC.
        object
            .upload(bytes)
            .await
            .map_err(|e| CoreError::Upload(format!("oss put_object failed: {e}")))?;

        let (url, expires_at) = self.presigned_get_url(&object_key)?;
        Ok(UploadedAsset {
            url,
            remote_id: object_key,
            expires_at: expires_at.to_rfc3339(),
        })
    }

    async fn cleanup(&self, remote_id: &str) -> Result<()> {
        let object = self.bucket.object(remote_id);
        if let Err(e) = object.delete().await {
            // Best-effort: surface as warn but DO NOT fail. The orchestrator
            // already passed terminal status; bubbling here just makes the UI
            // report a phantom error.
            tracing::warn!(remote_id = %remote_id, error = %e, "oss cleanup failed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_creds_json() -> &'static str {
        r#"{
          "oss": {
            "endpoint": "oss-cn-hangzhou.aliyuncs.com",
            "bucket": "mango-test",
            "access_key_id": "LTAI5tFAKEFAKEFAKE",
            "access_key_secret": "supersecret",
            "region": "cn-hangzhou",
            "url_expires_seconds": 3600
          }
        }"#
    }

    #[test]
    fn from_credentials_json_parses_full_block() {
        let u = OssUploader::from_credentials_json(sample_creds_json()).unwrap();
        assert_eq!(u.endpoint, "oss-cn-hangzhou.aliyuncs.com");
        assert_eq!(u.bucket_name, "mango-test");
        assert_eq!(u.access_key_id, "LTAI5tFAKEFAKEFAKE");
        assert_eq!(u.url_expires_seconds, 3600);
        assert_eq!(u.object_prefix, DEFAULT_PREFIX);
    }

    #[test]
    fn from_credentials_json_rejects_missing_oss_block() {
        let r = OssUploader::from_credentials_json("{}");
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn from_credentials_json_rejects_bogus_endpoint() {
        let bad = r#"{"oss":{"endpoint":"https://example.com","bucket":"b",
                              "access_key_id":"a","access_key_secret":"s"}}"#;
        let r = OssUploader::from_credentials_json(bad);
        assert!(matches!(r, Err(CoreError::Validation(_))));
    }

    #[test]
    fn presigned_url_contains_expected_query_params() {
        let u = OssUploader::from_credentials_json(sample_creds_json()).unwrap();
        let (url, expires_at) = u.presigned_get_url("mango/refs/20260525/abc.png").unwrap();
        assert!(url.starts_with("https://mango-test.oss-cn-hangzhou.aliyuncs.com/"));
        assert!(url.contains("OSSAccessKeyId=LTAI5tFAKEFAKEFAKE"));
        assert!(url.contains("Expires="));
        assert!(url.contains("Signature="));
        // Future timestamp.
        assert!(expires_at > Utc::now());
    }

    #[test]
    fn presigned_url_signature_matches_v1_recipe() {
        // Reference computation: HMAC-SHA1(secret, "GET\n\n\n{exp}\n/{bucket}/{key}")
        // base64-encoded.
        let u = OssUploader::new(
            "oss-cn-hangzhou.aliyuncs.com".into(),
            "mango-test".into(),
            "id".into(),
            "supersecret".into(),
            3600,
            None,
        )
        .unwrap();
        let key = "mango/refs/x.png";
        let (url, expires_at) = u.presigned_get_url(key).unwrap();
        let exp_unix = expires_at.timestamp();

        let s = format!("GET\n\n\n{exp_unix}\n/mango-test/{key}");
        let mut mac = HmacSha1::new_from_slice(b"supersecret").unwrap();
        mac.update(s.as_bytes());
        let want_sig = B64.encode(mac.finalize().into_bytes());
        let want_sig_enc =
            utf8_percent_encode(&want_sig, SIGNATURE_ENCODE_SET).to_string();
        assert!(url.contains(&format!("Signature={want_sig_enc}")));
    }

    #[test]
    fn build_object_key_uses_sha256_prefix_and_date() {
        let u = OssUploader::from_credentials_json(sample_creds_json()).unwrap();
        let key = u.build_object_key(b"hello", Path::new("ref.png"));
        // "hello" sha256 starts with 2cf24dba5fb0a30e
        assert!(
            key.contains("/2cf24dba5fb0a30e"),
            "expected sha-prefixed key, got {key}"
        );
        assert!(key.starts_with("mango/refs/"));
        assert!(key.ends_with(".png"));
    }
}
