use clap::Parser;
use s3fcp::{cli::Args, downloader};

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = downloader::download_and_stream(args).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
