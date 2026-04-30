use clap::Parser;
use s3fcp::{
    cli::{Cli, DownloadArgs},
    downloader::download_to_stdout,
    http_client::HttpClient,
    s3_client::S3Client,
};
use std::sync::Arc;
use url::Url;

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

    let result = match url.scheme() {
        "s3" => {
            let bucket = url.host_str().expect("Missing bucket");
            let key = url.path();

            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            let download_args = DownloadArgs::from(&cli);
            let client = Arc::new(S3Client::new(
                aws_sdk_s3::Client::new(&config),
                bucket,
                key,
                cli.version_id,
            ));

            download_to_stdout(client, download_args).await
        }
        "http" | "https" => {
            let client = Arc::new(HttpClient::new(url.to_string()));
            download_to_stdout(client, DownloadArgs::from(&cli)).await
        }
        scheme => {
            eprintln!("Unsupported URL scheme: {scheme}");
            std::process::exit(1)
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
