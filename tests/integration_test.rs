use aws_config::BehaviorVersion;
use aws_sdk_s3::config::{Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use s3fcp::cli::DownloadArgs;
use s3fcp::downloader::download;
use s3fcp::s3_client::S3Client;
use std::sync::Arc;
use testcontainers::{runners::AsyncRunner, ContainerAsync, ImageExt};
use testcontainers_modules::localstack::LocalStack;
use tokio::sync::OnceCell;

/// Shared LocalStack container for all tests
static LOCALSTACK: OnceCell<Arc<ContainerAsync<LocalStack>>> = OnceCell::const_new();

/// Get or initialize the shared LocalStack container
async fn get_localstack() -> Arc<ContainerAsync<LocalStack>> {
    LOCALSTACK
        .get_or_init(|| async {
            let container = LocalStack::default()
                .with_env_var("SERVICES", "s3")
                .start()
                .await
                .expect("Failed to start LocalStack");
            Arc::new(container)
        })
        .await
        .clone()
}

/// Helper to create an S3 client configured for LocalStack
async fn create_test_client() -> (Client, String) {
    let localstack = get_localstack().await;
    let port = localstack
        .get_host_port_ipv4(4566)
        .await
        .expect("Failed to get port");
    let endpoint_url = format!("http://127.0.0.1:{}", port);

    let credentials = Credentials::new("test", "test", None, None, "test");

    let config = aws_config::defaults(BehaviorVersion::latest())
        .credentials_provider(credentials)
        .region(Region::new("us-east-1"))
        .endpoint_url(endpoint_url.clone())
        .load()
        .await;

    (Client::new(&config), endpoint_url)
}

/// Helper to create an s3fcp S3Client configured for LocalStack
async fn create_s3fcp_client(
    endpoint: &str,
    bucket: String,
    key: String,
) -> Arc<S3Client> {
    let credentials = Credentials::new("test", "test", None, None, "test");

    let config = aws_config::defaults(BehaviorVersion::latest())
        .credentials_provider(credentials)
        .region(Region::new("us-east-1"))
        .endpoint_url(endpoint)
        .load()
        .await;

    Arc::new(S3Client::new(
        aws_sdk_s3::Client::new(&config),
        bucket,
        key,
        None,
    ))
}

/// Upload test data to S3
async fn upload_test_file(
    client: &Client,
    bucket: &str,
    key: &str,
    content: Vec<u8>,
) -> anyhow::Result<()> {
    // Create bucket
    client
        .create_bucket()
        .bucket(bucket)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create bucket: {}", e))?;

    // Upload file
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from(content))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to upload file: {}", e))?;

    Ok(())
}

#[tokio::test]
async fn test_download_small_file() -> anyhow::Result<()> {
    let (client, endpoint) = create_test_client().await;
    let bucket = "test-download-bucket";
    let key = "test-file.txt";
    let test_content = b"Hello from s3fcp integration test!".to_vec();

    // Upload test file
    upload_test_file(&client, bucket, key, test_content.clone()).await?;

    // Create s3fcp client
    let s3fcp_client =
        create_s3fcp_client(&endpoint, bucket.to_string(), key.to_string()).await;

    let args = DownloadArgs::builder().concurrency(2).quiet(true).build();

    // Download to a buffer
    let output = download(s3fcp_client, args, Vec::new()).await?;

    // Verify downloaded content matches
    assert_eq!(output, test_content);

    Ok(())
}

#[tokio::test]
async fn test_download_large_file_with_chunks() -> anyhow::Result<()> {
    let (client, endpoint) = create_test_client().await;
    let bucket = "test-chunked-bucket";
    let key = "large-file.bin";

    // Create a 10MB test file with pattern
    let chunk_size = 1024 * 1024; // 1MB
    let mut test_content = Vec::with_capacity(chunk_size * 10);
    for i in 0..(chunk_size * 10) {
        test_content.push((i % 256) as u8);
    }

    // Upload test file
    upload_test_file(&client, bucket, key, test_content.clone()).await?;

    // Create s3fcp client
    let s3fcp_client =
        create_s3fcp_client(&endpoint, bucket.to_string(), key.to_string()).await;

    // Download with smaller chunks to test chunking logic
    let args = DownloadArgs::builder()
        .concurrency(4)
        .chunk_size(2 * 1024 * 1024) // 2MB chunks - should create 5 chunks
        .quiet(true)
        .build();

    let output = download(s3fcp_client, args, Vec::new()).await?;

    // Verify downloaded content matches exactly
    assert_eq!(output.len(), test_content.len());
    assert_eq!(output, test_content);

    Ok(())
}

#[tokio::test]
async fn test_download_empty_file() -> anyhow::Result<()> {
    let (client, endpoint) = create_test_client().await;
    let bucket = "test-empty-bucket";
    let key = "empty.txt";
    let test_content = Vec::new();

    // Upload empty file
    upload_test_file(&client, bucket, key, test_content.clone()).await?;

    // Create s3fcp client
    let s3fcp_client =
        create_s3fcp_client(&endpoint, bucket.to_string(), key.to_string()).await;

    let args = DownloadArgs::builder().quiet(true).build();

    let output = download(s3fcp_client, args, Vec::new()).await?;

    // Verify empty output
    assert_eq!(output, test_content);

    Ok(())
}

#[tokio::test]
async fn test_download_single_byte() -> anyhow::Result<()> {
    let (client, endpoint) = create_test_client().await;
    let bucket = "test-single-byte-bucket";
    let key = "single.bin";
    let test_content = vec![42u8];

    upload_test_file(&client, bucket, key, test_content.clone()).await?;

    // Create s3fcp client
    let s3fcp_client =
        create_s3fcp_client(&endpoint, bucket.to_string(), key.to_string()).await;

    let args = DownloadArgs::builder().concurrency(1).quiet(true).build();

    let output = download(s3fcp_client, args, Vec::new()).await?;

    assert_eq!(output, test_content);

    Ok(())
}
