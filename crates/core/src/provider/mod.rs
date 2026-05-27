//! Provider abstraction. The task engine is wired against [`traits::ModelProvider`];
//! concrete impls (Bailian, etc.) live in separate modules added by spec-17+.

pub mod asset_uploader;
pub mod bailian;
pub mod error;
pub mod registry;
pub mod traits;

#[cfg(test)]
pub mod stub;

pub use asset_uploader::{AssetUploader, OssUploader, UploadedAsset};
pub use error::{ProviderErrorDetail, ProviderErrorKind};
pub use registry::{ProviderRegistry, ProviderRegistryBuilder};
pub use traits::{
    GenerationParams, ModelProvider, PollOutcome, ProviderCredentials, ProviderTaskStatus,
    SubmitOutcome, UploadSummary,
};
