mod chunk;
mod cli;
mod downloader;
mod error;
mod progress;
mod s3_client;
mod stage1_queue;
mod stage2_download;
mod stage3_output;
mod uri;

use clap::Parser;
use cli::Args;

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Err(e) = downloader::download_and_stream(args).await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
