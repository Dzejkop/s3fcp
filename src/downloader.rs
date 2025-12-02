use crate::chunk::create_chunks;
use crate::cli::Args;
use crate::error::Result;
use crate::progress::ProgressTracker;
use crate::s3_client::S3Client;
use crate::stage1_queue::queue_chunks;
use crate::stage2_download::download_worker;
use crate::stage3_output::ordered_output_writer;
use crate::uri::S3Uri;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub async fn download_and_stream(args: Args) -> Result<()> {
    // 1. Parse S3 URI
    let uri = S3Uri::parse(&args.s3_uri)?;

    // 2. Initialize AWS S3 client
    let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let client = Arc::new(S3Client::new(aws_sdk_s3::Client::new(&config)));

    // 3. HEAD request to get content_length
    let metadata = client
        .head_object(&uri.bucket, &uri.key, args.version_id.clone())
        .await?;

    // 4. Handle edge case: empty file
    if metadata.content_length == 0 {
        return Ok(());
    }

    // 5. Create chunks
    let chunks = create_chunks(metadata.content_length, args.chunk_size, args.version_id);
    let total_chunks = chunks.len();

    // 6. Handle edge case: single chunk - direct download
    if total_chunks == 1 {
        let chunk = &chunks[0];
        let data = client
            .get_object_range(
                &uri.bucket,
                &uri.key,
                chunk.start,
                chunk.end,
                chunk.version_id.clone(),
            )
            .await?;

        use tokio::io::AsyncWriteExt;
        tokio::io::stdout().write_all(&data).await?;
        tokio::io::stdout().flush().await?;
        return Ok(());
    }

    // 7. Setup progress tracker
    let progress = ProgressTracker::new(metadata.content_length, args.quiet);

    // 8. Setup channels for the 3 stages
    let (chunk_tx, chunk_rx) = mpsc::channel(args.concurrency);
    let (output_tx, output_rx) = mpsc::channel(args.concurrency * 2);

    // Wrap receiver in Arc<Mutex<>> for sharing among workers
    let chunk_rx = Arc::new(Mutex::new(chunk_rx));

    // 9. Spawn Stage 1: Queue
    let queue_handle = tokio::spawn(queue_chunks(chunks, chunk_tx));

    // 10. Spawn Stage 2: Download workers (worker pool)
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

    // 11. Spawn Stage 3: Ordered output
    let output_handle = tokio::spawn(ordered_output_writer(output_rx, total_chunks));

    // 12. Await Stage 1 completion and drop sender
    queue_handle.await??;
    // Sender is dropped here, so workers will receive None and exit

    // 13. Await all workers
    for handle in download_handles {
        handle.await??;
    }
    // Drop output sender so output writer knows when to stop
    drop(output_tx);

    // 14. Await output stage completion
    output_handle.await??;

    // 15. Finish progress
    progress.finish();

    Ok(())
}
