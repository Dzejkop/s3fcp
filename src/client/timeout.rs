use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use crate::client::{DownloadClient, ObjectMetadata};
use crate::error::{Result, S3FcpError};

pub struct TimeoutClient {
    inner: Arc<dyn DownloadClient>,
    timeout: Duration,
}

impl TimeoutClient {
    #[must_use]
    pub const fn new(inner: Arc<dyn DownloadClient>, timeout: Duration) -> Self {
        Self { inner, timeout }
    }

    fn timeout_error(&self, operation: &str) -> S3FcpError {
        S3FcpError::DownloadFailed(format!(
            "{operation} timed out after {}",
            humantime::format_duration(self.timeout)
        ))
    }
}

#[async_trait]
impl DownloadClient for TimeoutClient {
    async fn head(&self) -> Result<ObjectMetadata> {
        timeout(self.timeout, self.inner.head())
            .await
            .map_err(|_| self.timeout_error("HEAD"))?
    }

    async fn get_range(&self, start: u64, end: u64) -> Result<Bytes> {
        timeout(self.timeout, self.inner.get_range(start, end))
            .await
            .map_err(|_| self.timeout_error(&format!("GET range {start}-{end}")))?
    }

    async fn get_full(&self) -> Result<Bytes> {
        timeout(self.timeout, self.inner.get_full())
            .await
            .map_err(|_| self.timeout_error("GET"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    struct MockClient {
        delay: Duration,
    }

    #[async_trait]
    impl DownloadClient for MockClient {
        async fn head(&self) -> Result<ObjectMetadata> {
            sleep(self.delay).await;
            Ok(ObjectMetadata {
                content_length: 1,
                supports_range: true,
            })
        }

        async fn get_range(&self, _start: u64, _end: u64) -> Result<Bytes> {
            sleep(self.delay).await;
            Ok(Bytes::from_static(b"range"))
        }

        async fn get_full(&self) -> Result<Bytes> {
            sleep(self.delay).await;
            Ok(Bytes::from_static(b"full"))
        }
    }

    #[tokio::test]
    async fn times_out_slow_inner_client() {
        let client = TimeoutClient::new(
            Arc::new(MockClient {
                delay: Duration::from_millis(50),
            }),
            Duration::from_millis(5),
        );

        let err = client.get_range(0, 4).await.unwrap_err();

        assert!(
            matches!(err, S3FcpError::DownloadFailed(message) if message.contains("timed out"))
        );
    }
}
