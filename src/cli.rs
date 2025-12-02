use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "s3fcp")]
#[command(about = "Fast S3 object downloader with multi-part support", long_about = None)]
pub struct Args {
    /// S3 URI in the format s3://bucket/key
    pub s3_uri: String,

    /// S3 object version ID for versioned objects
    #[arg(long)]
    pub version_id: Option<String>,

    /// Number of concurrent download workers
    #[arg(short = 'c', long, default_value = "8")]
    pub concurrency: usize,

    /// Chunk size in bytes for range requests
    #[arg(long, default_value = "8388608")] // 8MB default
    pub chunk_size: usize,

    /// Quiet mode - suppress progress output
    #[arg(short = 'q', long)]
    pub quiet: bool,
}
