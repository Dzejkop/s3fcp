use clap::Parser;
use s3fcp::{
    cli::{Cli, DownloadArgs},
    client::{DownloadClient, HttpClient, S3Client, TimeoutClient},
    downloader::download_to_stdout,
    progress::{
        bar::ProgressBarTracker, logged::LoggedProgressTracker, quiet::QuietProgressTracker,
    },
};
use std::sync::Arc;
use url::Url;

fn with_timeout(
    client: Arc<dyn DownloadClient>,
    timeout: Option<std::time::Duration>,
) -> Arc<dyn DownloadClient> {
    match timeout {
        Some(timeout) => Arc::new(TimeoutClient::new(client, timeout)),
        None => client,
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let url: Url = match Url::parse(&cli.uri) {
        Ok(url) => url,
        Err(err) => {
            eprintln!("Invalid URL: {err}");
            std::process::exit(1);
        }
    };

    let progress = match (cli.quiet, cli.log_progress) {
        (true, _) => QuietProgressTracker::dyn_new(),
        (false, true) => LoggedProgressTracker::dyn_new(),
        (false, false) => ProgressBarTracker::dyn_new(),
    };

    let client: Arc<dyn DownloadClient> = match url.scheme() {
        "s3" => {
            let bucket = url.host_str().expect("Missing bucket");
            let key = url.path();
            let key = key.trim_start_matches('/'); // trim the leading backslash

            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            Arc::new(S3Client::new(
                aws_sdk_s3::Client::new(&config),
                bucket,
                key,
                cli.version_id.clone(),
            ))
        }
        "http" | "https" => Arc::new(HttpClient::new(url.to_string())),
        scheme => {
            eprintln!("Unsupported URL scheme: {scheme}");
            std::process::exit(1)
        }
    };

    let client = with_timeout(client, cli.timeout);
    let result = download_to_stdout(client, progress, DownloadArgs::from(&cli)).await;

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
