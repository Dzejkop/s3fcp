use crate::chunk::DownloadedChunk;
use crate::error::Result;
use std::collections::BTreeMap;
use tokio::io::{self, AsyncWriteExt};
use tokio::sync::mpsc;

/// Stage 3: Ordered output writer
/// Receives chunks (potentially out of order) and writes them to stdout in correct order
pub async fn ordered_output_writer(
    mut rx: mpsc::Receiver<DownloadedChunk>,
    total_chunks: usize,
) -> Result<()> {
    let mut buffer: BTreeMap<usize, DownloadedChunk> = BTreeMap::new();
    let mut next_expected = 0;
    let mut stdout = io::stdout();

    while let Some(chunk) = rx.recv().await {
        // Insert the chunk into the buffer
        buffer.insert(chunk.index, chunk);

        // Drain all sequential chunks starting from next_expected
        while let Some(chunk) = buffer.remove(&next_expected) {
            stdout.write_all(&chunk.data).await?;
            next_expected += 1;

            // If we've written all chunks, we're done
            if next_expected == total_chunks {
                stdout.flush().await?;
                return Ok(());
            }
        }
    }

    // Ensure all data is flushed
    stdout.flush().await?;
    Ok(())
}
