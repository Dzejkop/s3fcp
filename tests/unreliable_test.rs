use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::get,
    Router,
};
use bytes::Bytes;
use futures::stream;
use rand::{random_bool, random_range};
use s3fcp::cli::DownloadArgs;
use s3fcp::client::HttpClient;
use s3fcp::downloader::download;
use s3fcp::progress::quiet::QuietProgressTracker;
use std::sync::{atomic::Ordering, Arc};
use std::{io, sync::atomic::AtomicUsize};
use tokio::net::TcpListener;

#[derive(Clone)]
struct TestServerState {
    data: Arc<Vec<u8>>,
    fail_count: Arc<AtomicUsize>,
}

fn pseudo_random_bytes(len: usize) -> Vec<u8> {
    let mut state = 0x1234_5678_9abc_def0_u64;

    (0..len)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            (state >> 32) as u8
        })
        .collect()
}

async fn head_handler(State(state): State<TestServerState>) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_LENGTH, state.data.len().to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .body(Body::empty())
        .unwrap()
}

async fn get_handler(State(state): State<TestServerState>, headers: HeaderMap) -> Response {
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("bytes="))
        .unwrap();

    let (start, end) = range.split_once('-').unwrap();
    let start = start.parse::<u64>().unwrap();
    let end = end.parse::<u64>().unwrap();

    let bytes = state.data[start as usize..=end as usize].to_vec();
    let should_fail = random_bool(0.2); // 20% of chunks will fail

    let chunks = bytes
        .chunks(32)
        .map(|chunk| Ok::<_, io::Error>(Bytes::copy_from_slice(chunk)))
        .collect::<Vec<_>>();

    let random_valid_chunks = random_range(1..=3);

    let body = if should_fail {
        state.fail_count.fetch_add(1, Ordering::Relaxed);
        Body::from_stream(stream::iter(
            chunks
                .into_iter()
                .take(random_valid_chunks)
                .chain(std::iter::once(Err(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "simulated connection reset",
                )))),
        ))
    } else {
        Body::from_stream(stream::iter(chunks))
    };

    Response::builder()
        .status(StatusCode::PARTIAL_CONTENT)
        .header(header::CONTENT_LENGTH, (end - start + 1).to_string())
        .header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{}", state.data.len()),
        )
        .header(header::ACCEPT_RANGES, "bytes")
        .body(body)
        .unwrap()
}

async fn start_unreliable_server(data: Vec<u8>) -> (String, Arc<AtomicUsize>) {
    let fail_count = Arc::new(AtomicUsize::new(0));

    let state = TestServerState {
        data: Arc::new(data),
        fail_count: fail_count.clone(),
    };

    let app = Router::new()
        .route("/", get(get_handler).head(head_handler))
        .with_state(state);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{addr}/"), fail_count)
}

#[tokio::test]
async fn test_unreliable_download_test() -> anyhow::Result<()> {
    let content = pseudo_random_bytes(1024 * 1024); // 1 MiB
    let (url, fail_count) = start_unreliable_server(content.clone()).await;

    let client = Arc::new(HttpClient::new(url));
    let args = DownloadArgs::builder()
        .concurrency(10)
        .chunk_size(1024) // 1 KiB
        .build();
    let output = download(client, QuietProgressTracker::dyn_new(), args, Vec::new()).await?;

    println!("Fail count is {}", fail_count.load(Ordering::Relaxed));
    assert!(fail_count.load(Ordering::Relaxed) > 0);

    assert_eq!(output, content);
    Ok(())
}
