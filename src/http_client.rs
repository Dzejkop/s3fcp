use async_trait::async_trait;
use bytes::Bytes;
use reqwest::header::{ACCEPT_RANGES, CONTENT_LENGTH, RANGE};
use reqwest::Client;

use crate::error::{Result, S3FcpError};
use crate::s3_client::{DownloadClient, ObjectMetadata};

pub struct HttpClient {
    client: Client,
    url: String,
}

impl HttpClient {
    pub fn new(url: String) -> Self {
        Self {
            client: Client::new(),
            url,
        }
    }
}

#[async_trait]
impl DownloadClient for HttpClient {
    async fn head(&self) -> Result<ObjectMetadata> {
        let response = self.client.head(&self.url).send().await?;

        if !response.status().is_success() {
            return Err(S3FcpError::HttpError(format!(
                "HEAD request failed with status: {}",
                response.status()
            )));
        }

        let content_length = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| S3FcpError::HttpError("Missing Content-Length header".to_string()))?;

        let supports_range = response
            .headers()
            .get(ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "bytes")
            .unwrap_or(false);

        Ok(ObjectMetadata {
            content_length,
            supports_range,
        })
    }

    async fn get_range(&self, start: u64, end: u64) -> Result<Bytes> {
        let range = format!("bytes={}-{}", start, end);
        let response = self
            .client
            .get(&self.url)
            .header(RANGE, range)
            .send()
            .await?;

        // Check for 206 Partial Content
        if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(S3FcpError::HttpError(format!(
                "Expected 206 Partial Content, got {}",
                response.status()
            )));
        }

        Ok(response.bytes().await?)
    }

    async fn get_full(&self) -> Result<Bytes> {
        let response = self.client.get(&self.url).send().await?;

        if !response.status().is_success() {
            return Err(S3FcpError::HttpError(format!(
                "GET request failed with status: {}",
                response.status()
            )));
        }

        Ok(response.bytes().await?)
    }
}
