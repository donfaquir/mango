//! Provider abstraction. The task engine is wired against [`traits::ModelProvider`];
//! concrete impls (Bailian, etc.) live in separate modules added by spec-17+.

pub mod asset_uploader;
pub mod bailian;
pub mod registry;
pub mod traits;

#[cfg(test)]
pub mod stub;

pub use asset_uploader::{AssetUploader, OssUploader, UploadedAsset};
pub use registry::{ProviderRegistry, ProviderRegistryBuilder};
pub use traits::{GenerationParams, ModelProvider, ProviderCredentials, ProviderTaskStatus};
