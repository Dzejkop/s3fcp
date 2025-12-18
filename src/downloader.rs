use crate::chunk::{create_chunks, Chunk, DownloadedChunk};
use crate::cli::DownloadArgs;
use crate::error::Result;
use crate::progress::ProgressTracker;
use crate::s3_client::DownloadClient;
use backon::{ExponentialBuilder, Retryable};
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::io::{self, AsyncWriteExt};

/// Stage 1: Queue up download jobs
/// Sends chunks to a bounded channel, providing natural backpressure
async fn queue_chunks(chunks: Vec<Chunk>, tx: flume::Sender<Chunk>) -> Result<()> {
    for chunk in chunks {
        tx.send_async(chunk).await.map_err(|e| {
            crate::error::S3FcpError::DownloadFailed(format!("Failed to queue chunk: {}", e))
        })?;
    }
    Ok(())
}

/// Stage 2: Download worker
/// Pulls chunks from the queue and downloads them with retry logic
async fn download_worker(
    client: Arc<dyn DownloadClient>,
    rx: flume::Receiver<Chunk>,
    output_tx: flume::Sender<DownloadedChunk>,
    progress: Arc<ProgressTracker>,
) -> Result<()> {
    while let Ok(chunk) = rx.recv_async().await {
        // Download with retry logic using backon
        let data = (|| async { client.get_range(chunk.start, chunk.end).await })
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
            .send_async(DownloadedChunk {
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

/// Stage 3: Ordered output writer
/// Receives chunks (potentially out of order) and writes them in correct order
async fn ordered_output_writer<W>(
    rx: flume::Receiver<DownloadedChunk>,
    total_chunks: usize,
    mut writer: W,
) -> Result<W>
where
    W: AsyncWriteExt + Unpin,
{
    let mut buffer: BTreeMap<usize, DownloadedChunk> = BTreeMap::new();
    let mut next_expected = 0;

    while let Ok(chunk) = rx.recv_async().await {
        // Insert the chunk into the buffer
        buffer.insert(chunk.index, chunk);

        // Drain all sequential chunks starting from next_expected
        while let Some(chunk) = buffer.remove(&next_expected) {
            writer.write_all(&chunk.data).await?;
            next_expected += 1;

            // If we've written all chunks, we're done
            if next_expected == total_chunks {
                writer.flush().await?;
                return Ok(writer);
            }
        }
    }

    // Ensure all data is flushed
    writer.flush().await?;
    Ok(writer)
}

/// Download using chunked parallel requests
pub async fn download_chunked<W>(
    client: Arc<dyn DownloadClient>,
    args: DownloadArgs,
    content_length: u64,
    writer: W,
) -> Result<W>
where
    W: AsyncWriteExt + Unpin + Send + 'static,
{
    // Handle edge case: empty file
    if content_length == 0 {
        return Ok(writer);
    }

    // Create chunks
    let chunks = create_chunks(content_length, args.chunk_size);
    let total_chunks = chunks.len();

    // Setup progress tracker
    let progress = ProgressTracker::new(content_length, args.quiet);

    // Setup channels for the 3 stages
    let (chunk_tx, chunk_rx) = flume::bounded(args.concurrency);
    let (output_tx, output_rx) = flume::bounded(args.concurrency * 2);

    // Spawn Stage 1: Queue
    let queue_handle = tokio::spawn(queue_chunks(chunks, chunk_tx));

    // Spawn Stage 2: Download workers (worker pool)
    let mut download_handles = vec![];
    for _ in 0..args.concurrency {
        let worker_handle = tokio::spawn(download_worker(
            client.clone(),
            chunk_rx.clone(),
            output_tx.clone(),
            progress.clone(),
        ));
        download_handles.push(worker_handle);
    }

    // Spawn Stage 3: Ordered output
    let output_handle = tokio::spawn(ordered_output_writer(output_rx, total_chunks, writer));

    // Await Stage 1 completion and drop sender
    queue_handle.await??;
    // Sender is dropped here, so workers will receive Err and exit

    // Await all workers
    for handle in download_handles {
        handle.await??;
    }
    // Drop output sender so output writer knows when to stop
    drop(output_tx);

    // Await output stage completion and get writer back
    let writer = output_handle.await??;

    // Finish progress
    progress.finish();

    Ok(writer)
}

/// Download using a single stream (for servers without Range support)
pub async fn download_single_stream<W>(
    client: Arc<dyn DownloadClient>,
    content_length: u64,
    quiet: bool,
    mut writer: W,
) -> Result<W>
where
    W: AsyncWriteExt + Unpin,
{
    // Handle edge case: empty file
    if content_length == 0 {
        return Ok(writer);
    }

    let progress = ProgressTracker::new(content_length, quiet);

    // Download entire file in a single request
    let data = client.get_full().await?;
    progress.increment(data.len() as u64);
    writer.write_all(&data).await?;
    writer.flush().await?;

    progress.finish();

    Ok(writer)
}

/// Main download function - chooses strategy based on server capabilities
pub async fn download<W>(
    client: Arc<dyn DownloadClient>,
    args: DownloadArgs,
    writer: W,
) -> Result<W>
where
    W: AsyncWriteExt + Unpin + Send + 'static,
{
    // HEAD request to get content_length and check Range support
    let metadata = client.head().await?;

    if metadata.supports_range {
        download_chunked(client, args, metadata.content_length, writer).await
    } else {
        download_single_stream(client, metadata.content_length, args.quiet, writer).await
    }
}

pub async fn download_to_stdout(
    client: Arc<dyn DownloadClient>,
    args: DownloadArgs,
) -> Result<()> {
    download(client, args, io::stdout()).await?;
    Ok(())
}
