use crate::chunk::{create_chunks, Chunk, DownloadedChunk};
use crate::cli::DownloadArgs;
use crate::client::DownloadClient;
use crate::error::Result;
use crate::progress::ProgressTracker;
use backon::{ExponentialBuilder, Retryable};
use human_bytes::human_bytes;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::io::{self, AsyncWriteExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;

struct ScheduledChunk {
    chunk: Chunk,
    permit: OwnedSemaphorePermit,
}

struct BufferedChunk {
    chunk: DownloadedChunk,
    _permit: OwnedSemaphorePermit,
}

#[allow(clippy::cast_precision_loss)]
const fn bytes_to_f64(bytes: u64) -> f64 {
    bytes as f64
}

/// Stage 1: Queue up download jobs
/// Sends chunks to a bounded channel, providing natural backpressure
async fn queue_chunks(
    chunks: Vec<Chunk>,
    tx: flume::Sender<ScheduledChunk>,
    window: Arc<Semaphore>,
) -> Result<()> {
    for chunk in chunks {
        let permit = window.clone().acquire_owned().await.map_err(|e| {
            crate::error::S3FcpError::DownloadFailed(format!("Failed to acquire chunk permit: {e}"))
        })?;
        tx.send_async(ScheduledChunk { chunk, permit })
            .await
            .map_err(|e| {
                crate::error::S3FcpError::DownloadFailed(format!("Failed to queue chunk: {e}"))
            })?;
    }
    Ok(())
}

/// Stage 2: Download worker
/// Pulls chunks from the queue and downloads them with retry logic
async fn download_worker(
    client: Arc<dyn DownloadClient>,
    progress: Arc<dyn ProgressTracker>,
    rx: flume::Receiver<ScheduledChunk>,
    output_tx: flume::Sender<BufferedChunk>,
    verbose: bool,
) -> Result<()> {
    while let Ok(scheduled) = rx.recv_async().await {
        let chunk = scheduled.chunk;
        // Download with retry logic using backon
        let chunk_index = chunk.index;
        let chunk_start = chunk.start;
        let chunk_end = chunk.end;
        let download_started_at = verbose.then(std::time::Instant::now);
        let data = (|| async { client.get_range(chunk.start, chunk.end).await })
            .retry(
                ExponentialBuilder::default()
                    .with_max_times(10)
                    .with_min_delay(std::time::Duration::from_millis(100))
                    .with_max_delay(std::time::Duration::from_secs(5)),
            )
            .notify(move |err, duration| {
                eprintln!(
                    "Failed to download chunk {chunk_index}: {err}; retrying in {duration:?}"
                );
            })
            .await?;

        if let Some(download_started_at) = download_started_at {
            eprintln!(
                "Downloaded chunk {chunk_index} ({}-{}) in {:?}",
                human_bytes(bytes_to_f64(chunk_start)),
                human_bytes(bytes_to_f64(chunk_end)),
                download_started_at.elapsed()
            );
        }

        let data_len = data.len() as u64;
        progress.increment(data_len);

        output_tx
            .send_async(BufferedChunk {
                chunk: DownloadedChunk {
                    index: chunk.index,
                    data,
                },
                _permit: scheduled.permit,
            })
            .await
            .map_err(|e| {
                crate::error::S3FcpError::DownloadFailed(format!(
                    "Failed to send downloaded chunk: {e}"
                ))
            })?;
    }

    Ok(())
}

/// Stage 3: Ordered output writer
/// Receives chunks (potentially out of order) and writes them in correct order
async fn ordered_output_writer<W>(
    rx: flume::Receiver<BufferedChunk>,
    total_chunks: usize,
    mut writer: W,
    verbose: bool,
) -> Result<W>
where
    W: AsyncWriteExt + Unpin,
{
    let mut buffer: BTreeMap<usize, BufferedChunk> = BTreeMap::new();
    let mut next_expected = 0;

    while let Ok(chunk) = rx.recv_async().await {
        // Insert the chunk into the buffer
        buffer.insert(chunk.chunk.index, chunk);

        // Drain all sequential chunks starting from next_expected
        while let Some(chunk) = buffer.remove(&next_expected) {
            if verbose {
                eprintln!("Writing chunk {}", chunk.chunk.index);
            }
            writer.write_all(&chunk.chunk.data).await?;
            if verbose {
                eprintln!("Wrote chunk {}", chunk.chunk.index);
            }
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

/// Download using chunked parallel requests.
///
/// # Errors
/// Returns an error if chunk scheduling, downloading, ordered writing, or task joining fails.
pub async fn download_chunked<W>(
    client: Arc<dyn DownloadClient>,
    progress: Arc<dyn ProgressTracker>,
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
    progress.reset(content_length);

    let concurrency = args.concurrency.max(1);
    let max_buffered_chunks = args.max_buffered_chunks.max(1);

    // Setup channels for the 3 stages
    let (chunk_tx, chunk_rx) = flume::bounded(concurrency);
    let (output_tx, output_rx) = flume::bounded(concurrency);
    let window = Arc::new(Semaphore::new(max_buffered_chunks));

    let mut tasks = JoinSet::new();

    // Spawn Stage 1: Queue
    tasks.spawn(queue_chunks(chunks, chunk_tx, window));

    // Spawn Stage 2: Download workers (worker pool)
    for _ in 0..concurrency {
        tasks.spawn(download_worker(
            client.clone(),
            progress.clone(),
            chunk_rx.clone(),
            output_tx.clone(),
            args.verbose,
        ));
    }

    // Spawn Stage 3: Ordered output
    let output_handle = tokio::spawn(ordered_output_writer(
        output_rx,
        total_chunks,
        writer,
        args.verbose,
    ));

    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tasks.abort_all();
                output_handle.abort();
                return Err(e);
            }
            Err(e) => {
                tasks.abort_all();
                output_handle.abort();
                return Err(e.into());
            }
        }
    }

    // Drop output sender so output writer knows when to stop
    drop(output_tx);

    // Await output stage completion and get writer back
    let writer = output_handle.await??;

    // Finish progress
    progress.finish();

    Ok(writer)
}

/// Download using a single stream (for servers without Range support).
///
/// # Errors
/// Returns an error if the full download or output write fails.
pub async fn download_single_stream<W>(
    client: Arc<dyn DownloadClient>,
    progress: Arc<dyn ProgressTracker>,
    content_length: u64,
    mut writer: W,
) -> Result<W>
where
    W: AsyncWriteExt + Unpin,
{
    // Handle edge case: empty file
    if content_length == 0 {
        return Ok(writer);
    }

    progress.reset(content_length);

    // Download entire file in a single request
    let data = client.get_full().await?;
    progress.increment(data.len() as u64);
    writer.write_all(&data).await?;
    writer.flush().await?;

    progress.finish();

    Ok(writer)
}

/// Main download function - chooses strategy based on server capabilities.
///
/// # Errors
/// Returns an error if metadata lookup or the selected download strategy fails.
pub async fn download<W>(
    client: Arc<dyn DownloadClient>,
    progress: Arc<dyn ProgressTracker>,
    args: DownloadArgs,
    writer: W,
) -> Result<W>
where
    W: AsyncWriteExt + Unpin + Send + 'static,
{
    // HEAD request to get content_length and check Range support
    let metadata = client.head().await?;

    if metadata.supports_range {
        download_chunked(client, progress, args, metadata.content_length, writer).await
    } else {
        download_single_stream(client, progress, metadata.content_length, writer).await
    }
}

/// Download to stdout.
///
/// # Errors
/// Returns an error if the download or stdout write fails.
pub async fn download_to_stdout(
    client: Arc<dyn DownloadClient>,
    progress: Arc<dyn ProgressTracker>,
    args: DownloadArgs,
) -> Result<()> {
    download(client, progress, args, io::stdout()).await?;
    Ok(())
}
