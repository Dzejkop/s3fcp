use crate::chunk::Chunk;
use crate::error::Result;
use tokio::sync::mpsc;

/// Stage 1: Queue up download jobs
/// Sends chunks to a bounded channel, providing natural backpressure
pub async fn queue_chunks(chunks: Vec<Chunk>, tx: mpsc::Sender<Chunk>) -> Result<()> {
    for chunk in chunks {
        tx.send(chunk).await.map_err(|e| {
            crate::error::S3FcpError::DownloadFailed(format!("Failed to queue chunk: {}", e))
        })?;
    }
    Ok(())
}
