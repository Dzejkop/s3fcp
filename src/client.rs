use async_trait::async_trait;
use bytes::Bytes;

use crate::error::Result;

pub mod http;
pub mod s3;
pub mod timeout;

pub use http::HttpClient;
pub use s3::S3Client;
pub use timeout::TimeoutClient;

pub struct ObjectMetadata {
    pub content_length: u64,
    pub supports_range: bool,
}

#[async_trait]
pub trait DownloadClient: Send + Sync {
    async fn head(&self) -> Result<ObjectMetadata>;
    async fn get_range(&self, start: u64, end: u64) -> Result<Bytes>;
    async fn get_full(&self) -> Result<Bytes>;
}
