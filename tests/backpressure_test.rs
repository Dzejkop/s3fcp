use async_trait::async_trait;
use bytes::Bytes;
use s3fcp::cli::DownloadArgs;
use s3fcp::downloader::download;
use s3fcp::error::Result;
use s3fcp::s3_client::{DownloadClient, ObjectMetadata};
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, timeout, Duration};

#[derive(Clone)]
struct BlockingFirstChunkClient {
    started: Arc<Mutex<BTreeSet<u64>>>,
}

impl BlockingFirstChunkClient {
    const fn new(started: Arc<Mutex<BTreeSet<u64>>>) -> Self {
        Self { started }
    }
}

#[async_trait]
impl DownloadClient for BlockingFirstChunkClient {
    async fn head(&self) -> Result<ObjectMetadata> {
        Ok(ObjectMetadata {
            content_length: 10,
            supports_range: true,
        })
    }

    async fn get_range(&self, start: u64, _end: u64) -> Result<Bytes> {
        self.started.lock().unwrap().insert(start);

        if start == 0 {
            futures::future::pending().await
        } else {
            Ok(Bytes::from_static(&[1]))
        }
    }

    async fn get_full(&self) -> Result<Bytes> {
        unreachable!()
    }
}

#[tokio::test]
async fn chunk_downloads_are_backpressured_by_ordered_writer() -> anyhow::Result<()> {
    let started = Arc::new(Mutex::new(BTreeSet::new()));
    let client = Arc::new(BlockingFirstChunkClient::new(started.clone()));
    let args = DownloadArgs::builder()
        .concurrency(10)
        .chunk_size(1)
        .max_buffered_chunks(3)
        .quiet(true)
        .build();

    let download_task = tokio::spawn(download(client, args, Vec::new()));

    timeout(Duration::from_secs(1), async {
        loop {
            if started.lock().unwrap().len() >= 3 {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await?;

    sleep(Duration::from_millis(100)).await;

    let started = started.lock().unwrap().clone();
    assert_eq!(started, BTreeSet::from([0, 1, 2]));

    download_task.abort();

    Ok(())
}
