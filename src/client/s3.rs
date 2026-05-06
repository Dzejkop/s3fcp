use async_trait::async_trait;
use aws_sdk_s3::Client;
use bytes::Bytes;

use crate::client::{DownloadClient, ObjectMetadata};
use crate::error::{Result, S3FcpError};

pub struct S3Client {
    client: Client,
    bucket: String,
    key: String,
    version_id: Option<String>,
}

impl S3Client {
    #[must_use]
    pub fn new(
        client: Client,
        bucket: impl Into<String>,
        key: impl Into<String>,
        version_id: Option<impl Into<String>>,
    ) -> Self {
        Self {
            client,
            bucket: bucket.into(),
            key: key.into(),
            version_id: version_id.map(Into::into),
        }
    }
}

#[async_trait]
impl DownloadClient for S3Client {
    async fn head(&self) -> Result<ObjectMetadata> {
        let mut request = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&self.key);

        if let Some(version) = &self.version_id {
            request = request.version_id(version);
        }

        let response = request
            .send()
            .await
            .map_err(|e| S3FcpError::S3Error(format!("HEAD request failed: {e}")))?;

        let content_length = response
            .content_length()
            .ok_or_else(|| S3FcpError::S3Error("Content-Length header missing".to_string()))?;
        let content_length = u64::try_from(content_length)
            .map_err(|_| S3FcpError::S3Error("Content-Length cannot be negative".to_string()))?;

        Ok(ObjectMetadata {
            content_length,
            supports_range: true, // S3 always supports range requests
        })
    }

    async fn get_range(&self, start: u64, end: u64) -> Result<Bytes> {
        let range = format!("bytes={start}-{end}");
        let mut request = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&self.key)
            .range(range);

        if let Some(version) = &self.version_id {
            request = request.version_id(version);
        }

        let response = request
            .send()
            .await
            .map_err(|e| S3FcpError::S3Error(format!("GET request failed: {e}")))?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| S3FcpError::S3Error(format!("Failed to read response body: {e}")))?
            .into_bytes();

        Ok(data)
    }

    async fn get_full(&self) -> Result<Bytes> {
        let mut request = self.client.get_object().bucket(&self.bucket).key(&self.key);

        if let Some(version) = &self.version_id {
            request = request.version_id(version);
        }

        let response = request
            .send()
            .await
            .map_err(|e| S3FcpError::S3Error(format!("GET request failed: {e}")))?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| S3FcpError::S3Error(format!("Failed to read response body: {e}")))?
            .into_bytes();

        Ok(data)
    }
}
