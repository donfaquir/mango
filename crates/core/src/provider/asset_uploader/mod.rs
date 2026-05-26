//! Provider-agnostic uploader abstraction (spec-16).
//!
//! [`AssetUploader`] is the contract: take a local file, return a publicly
//! reachable URL + a handle for cleanup. Today the only implementation is
//! [`OssUploader`], used by the bailian (`happyhorse-1.0-r2v`) provider in
//! spec-17.

pub mod oss;
pub mod traits;

pub use oss::OssUploader;
pub use traits::{AssetUploader, UploadedAsset};
