use crate::chunk::{Chunk, DownloadedChunk};
use crate::error::Result;
use crate::progress::ProgressTracker;
use crate::s3_client::S3Client;
use backon::{ExponentialBuilder, Retryable};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// Stage 2: Download worker
/// Pulls chunks from the queue and downloads them with retry logic
pub async fn download_worker(
    client: Arc<S3Client>,
    bucket: String,
    key: String,
    rx: Arc<Mutex<mpsc::Receiver<Chunk>>>,
    output_tx: mpsc::Sender<DownloadedChunk>,
    progress: Arc<ProgressTracker>,
) -> Result<()> {
    loop {
        // Lock the receiver and try to get the next chunk
        let chunk = {
            let mut rx_guard = rx.lock().await;
            rx_guard.recv().await
        };

        // If no more chunks, exit
        let Some(chunk) = chunk else {
            break;
        };
        // Download with retry logic using backon
        let data = (|| async {
            client
                .get_object_range(
                    &bucket,
                    &key,
                    chunk.start,
                    chunk.end,
                    chunk.version_id.clone(),
                )
                .await
        })
        .retry(
            ExponentialBuilder::default()
                .with_max_times(3)
                .with_min_delay(std::time::Duration::from_millis(100))
                .with_max_delay(std::time::Duration::from_secs(5)),
        )
        .await?;

        let data_len = data.len() as u64;
        progress.increment(data_len);

        output_tx
            .send(DownloadedChunk {
                index: chunk.index,
                data,
            })
            .await
            .map_err(|e| {
                crate::error::S3FcpError::DownloadFailed(format!(
                    "Failed to send downloaded chunk: {}",
                    e
                ))
            })?;
    }

    Ok(())
}
