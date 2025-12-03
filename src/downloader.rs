use crate::chunk::{create_chunks, Chunk, DownloadedChunk};
use crate::cli::Args;
use crate::error::Result;
use crate::progress::ProgressTracker;
use crate::s3_client::S3Client;
use crate::uri::S3Uri;
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
    client: Arc<S3Client>,
    bucket: String,
    key: String,
    rx: flume::Receiver<Chunk>,
    output_tx: flume::Sender<DownloadedChunk>,
    progress: Arc<ProgressTracker>,
) -> Result<()> {
    while let Ok(chunk) = rx.recv_async().await {
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

pub async fn download_and_stream_to<W>(args: Args, writer: W) -> Result<W>
where
    W: AsyncWriteExt + Unpin + Send + 'static,
{
    // Parse S3 URI
    let uri = S3Uri::parse(&args.s3_uri)?;

    // Initialize AWS S3 client
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = Arc::new(S3Client::new(aws_sdk_s3::Client::new(&config)));

    // HEAD request to get content_length
    let metadata = client
        .head_object(&uri.bucket, &uri.key, args.version_id.clone())
        .await?;

    // Handle edge case: empty file
    if metadata.content_length == 0 {
        return Ok(writer);
    }

    // Create chunks
    let chunks = create_chunks(metadata.content_length, args.chunk_size, args.version_id);
    let total_chunks = chunks.len();

    // Setup progress tracker
    let progress = ProgressTracker::new(metadata.content_length, args.quiet);

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
            uri.bucket.clone(),
            uri.key.clone(),
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

pub async fn download_and_stream(args: Args) -> Result<()> {
    download_and_stream_to(args, io::stdout()).await?;
    Ok(())
}
