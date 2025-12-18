use clap::Parser;
use s3fcp::{
    cli::{Cli, Command, DownloadArgs},
    downloader::download_to_stdout,
    http_client::HttpClient,
    s3_client::S3Client,
    uri::{HttpUri, S3Uri},
};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::S3(args) => {
            let uri = match S3Uri::parse(&args.uri) {
                Ok(uri) => uri,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
            let download_args = DownloadArgs::from(&args);
            let client = Arc::new(S3Client::new(
                aws_sdk_s3::Client::new(&config),
                uri.bucket,
                uri.key,
                args.version_id,
            ));

            download_to_stdout(client, download_args).await
        }
        Command::Http(args) => {
            let uri = match HttpUri::parse(&args.url) {
                Ok(uri) => uri,
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            };

            let client = Arc::new(HttpClient::new(uri.url));

            download_to_stdout(client, DownloadArgs::from(&args)).await
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
