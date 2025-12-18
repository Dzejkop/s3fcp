use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "s3fcp")]
#[command(about = "Fast file downloader with multi-part support", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Download from S3
    S3(S3Args),
    /// Download from HTTP/HTTPS URL
    Http(HttpArgs),
}

#[derive(Args, Debug, Clone)]
pub struct S3Args {
    /// S3 URI in the format s3://bucket/key
    pub uri: String,

    /// S3 object version ID for versioned objects
    #[arg(long)]
    pub version_id: Option<String>,

    /// Number of concurrent download workers
    #[arg(short = 'c', long, default_value = "10")]
    pub concurrency: usize,

    /// Chunk size (supports human-readable sizes: 8MB, 16MiB, 1GB, etc.)
    #[arg(long, default_value = "8MB", value_parser = parse_chunk_size)]
    pub chunk_size: usize,

    /// Quiet mode - suppress progress output
    #[arg(short = 'q', long)]
    pub quiet: bool,
}

#[derive(Args, Debug, Clone)]
pub struct HttpArgs {
    /// HTTP/HTTPS URL to download
    pub url: String,

    /// Number of concurrent download workers
    #[arg(short = 'c', long, default_value = "10")]
    pub concurrency: usize,

    /// Chunk size (supports human-readable sizes: 8MB, 16MiB, 1GB, etc.)
    #[arg(long, default_value = "8MB", value_parser = parse_chunk_size)]
    pub chunk_size: usize,

    /// Quiet mode - suppress progress output
    #[arg(short = 'q', long)]
    pub quiet: bool,
}

/// Common download arguments shared between S3 and HTTP
#[derive(Debug, Clone, bon::Builder)]
pub struct DownloadArgs {
    #[builder(default = 10)]
    pub concurrency: usize,
    #[builder(default = 8 * 1024 * 1024)]
    pub chunk_size: usize,
    #[builder(default)]
    pub quiet: bool,
}

impl From<&S3Args> for DownloadArgs {
    fn from(args: &S3Args) -> Self {
        Self {
            concurrency: args.concurrency,
            chunk_size: args.chunk_size,
            quiet: args.quiet,
        }
    }
}

impl From<&HttpArgs> for DownloadArgs {
    fn from(args: &HttpArgs) -> Self {
        Self {
            concurrency: args.concurrency,
            chunk_size: args.chunk_size,
            quiet: args.quiet,
        }
    }
}

fn parse_chunk_size(s: &str) -> Result<usize, String> {
    let s = s.trim().to_uppercase();

    // Try to parse as plain number first
    if let Ok(num) = s.parse::<usize>() {
        return Ok(num);
    }

    // Extract number and suffix
    let (num_str, suffix) = s
        .char_indices()
        .find(|(_, c)| c.is_alphabetic())
        .map(|(i, _)| s.split_at(i))
        .ok_or_else(|| format!("Invalid size format: {}", s))?;

    let num: f64 = num_str
        .trim()
        .parse()
        .map_err(|_| format!("Invalid number: {}", num_str))?;

    let multiplier: u64 = match suffix.trim() {
        "B" => 1,
        "KB" | "K" => 1_000,
        "KIB" => 1_024,
        "MB" | "M" => 1_000_000,
        "MIB" => 1_048_576,
        "GB" | "G" => 1_000_000_000,
        "GIB" => 1_073_741_824,
        "TB" | "T" => 1_000_000_000_000,
        "TIB" => 1_099_511_627_776,
        _ => return Err(format!("Unknown size suffix: {}", suffix)),
    };

    Ok((num * multiplier as f64) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_chunk_size() {
        assert_eq!(parse_chunk_size("1024").unwrap(), 1024);
        assert_eq!(parse_chunk_size("8MB").unwrap(), 8_000_000);
        assert_eq!(parse_chunk_size("8MiB").unwrap(), 8_388_608);
        assert_eq!(parse_chunk_size("1GB").unwrap(), 1_000_000_000);
        assert_eq!(parse_chunk_size("1GiB").unwrap(), 1_073_741_824);
        assert_eq!(parse_chunk_size("16 MB").unwrap(), 16_000_000);
    }
}
