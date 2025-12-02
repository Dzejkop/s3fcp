use crate::error::{Result, S3FcpError};
use aws_sdk_s3::Client;
use bytes::Bytes;

pub struct S3Client {
    client: Client,
}

pub struct ObjectMetadata {
    pub content_length: u64,
}

impl S3Client {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Get object metadata via HEAD request
    pub async fn head_object(
        &self,
        bucket: &str,
        key: &str,
        version_id: Option<String>,
    ) -> Result<ObjectMetadata> {
        let mut request = self.client.head_object().bucket(bucket).key(key);

        if let Some(version) = version_id {
            request = request.version_id(version);
        }

        let response = request
            .send()
            .await
            .map_err(|e| S3FcpError::S3Error(format!("HEAD request failed: {}", e)))?;

        let content_length = response
            .content_length()
            .ok_or_else(|| S3FcpError::S3Error("Content-Length header missing".to_string()))?
            as u64;

        Ok(ObjectMetadata { content_length })
    }

    /// Download a byte range of an object
    pub async fn get_object_range(
        &self,
        bucket: &str,
        key: &str,
        start: u64,
        end: u64,
        version_id: Option<String>,
    ) -> Result<Bytes> {
        let range = format!("bytes={}-{}", start, end);
        let mut request = self
            .client
            .get_object()
            .bucket(bucket)
            .key(key)
            .range(range);

        if let Some(version) = version_id {
            request = request.version_id(version);
        }

        let response = request
            .send()
            .await
            .map_err(|e| S3FcpError::S3Error(format!("GET request failed: {}", e)))?;

        let data = response
            .body
            .collect()
            .await
            .map_err(|e| S3FcpError::S3Error(format!("Failed to read response body: {}", e)))?
            .into_bytes();

        Ok(data)
    }
}
