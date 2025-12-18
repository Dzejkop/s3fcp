use axum::Router;
use s3fcp::cli::DownloadArgs;
use s3fcp::downloader::download;
use s3fcp::http_client::HttpClient;
use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

fn test_args(concurrency: usize, chunk_size: usize) -> DownloadArgs {
    DownloadArgs {
        concurrency,
        chunk_size,
        quiet: true,
    }
}

/// Start a static file server and return (base_url, temp_dir)
async fn start_file_server() -> (String, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let serve_dir = ServeDir::new(temp_dir.path());
    let app = Router::new().fallback_service(serve_dir);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{}", addr), temp_dir)
}

/// Create a test file with given content
fn create_test_file(dir: &TempDir, name: &str, content: &[u8]) {
    let path = dir.path().join(name);
    let mut file = std::fs::File::create(path).unwrap();
    file.write_all(content).unwrap();
}

#[tokio::test]
async fn test_http_download_small_file() -> anyhow::Result<()> {
    let (base_url, temp_dir) = start_file_server().await;
    let content = b"Hello from HTTP integration test!";
    create_test_file(&temp_dir, "test.txt", content);

    let client = Arc::new(HttpClient::new(format!("{}/test.txt", base_url)));
    let output = download(client, test_args(2, 5 * 1024 * 1024), Vec::new()).await?;

    assert_eq!(output, content);
    Ok(())
}

#[tokio::test]
async fn test_http_download_chunked_with_range() -> anyhow::Result<()> {
    let (base_url, temp_dir) = start_file_server().await;

    // 1MB file with pattern
    let content: Vec<u8> = (0..1024 * 1024).map(|i| (i % 256) as u8).collect();
    create_test_file(&temp_dir, "large.bin", &content);

    let client = Arc::new(HttpClient::new(format!("{}/large.bin", base_url)));
    // Use 256KB chunks = 4 chunks for 1MB
    let output = download(client, test_args(4, 256 * 1024), Vec::new()).await?;

    assert_eq!(output.len(), content.len());
    assert_eq!(output, content);
    Ok(())
}

#[tokio::test]
async fn test_http_download_empty_file() -> anyhow::Result<()> {
    let (base_url, temp_dir) = start_file_server().await;
    create_test_file(&temp_dir, "empty.txt", b"");

    let client = Arc::new(HttpClient::new(format!("{}/empty.txt", base_url)));
    let output = download(client, test_args(2, 5 * 1024 * 1024), Vec::new()).await?;

    assert!(output.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_http_download_single_byte() -> anyhow::Result<()> {
    let (base_url, temp_dir) = start_file_server().await;
    create_test_file(&temp_dir, "single.bin", &[42u8]);

    let client = Arc::new(HttpClient::new(format!("{}/single.bin", base_url)));
    let output = download(client, test_args(1, 5 * 1024 * 1024), Vec::new()).await?;

    assert_eq!(output, vec![42u8]);
    Ok(())
}

#[tokio::test]
async fn test_http_download_404() -> anyhow::Result<()> {
    let (base_url, _temp_dir) = start_file_server().await;

    let client = Arc::new(HttpClient::new(format!("{}/not-found.txt", base_url)));
    let result = download(client, test_args(2, 5 * 1024 * 1024), Vec::new()).await;

    assert!(result.is_err());
    Ok(())
}
