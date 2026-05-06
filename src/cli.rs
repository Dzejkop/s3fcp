use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "s3fcp")]
#[command(about = "Fast file downloader with multi-part support", long_about = None)]
pub struct Cli {
    /// URI to download (s3://, http://, or https://)
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

    /// Maximum chunks that may be downloaded ahead of ordered writes
    #[arg(long, default_value_t = 512)]
    pub max_buffered_chunks: usize,

    /// Quiet mode - suppress progress output
    #[arg(short = 'q', long)]
    pub quiet: bool,

    /// Verbose mode - log per-chunk download timings
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Logs progress instead of a progress bar
    #[arg(short = 'l', long)]
    pub log_progress: bool,
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
    #[builder(default)]
    pub verbose: bool,
    #[builder(default = 512)]
    pub max_buffered_chunks: usize,
}

impl From<&Cli> for DownloadArgs {
    fn from(args: &Cli) -> Self {
        Self {
            concurrency: args.concurrency,
            chunk_size: args.chunk_size,
            quiet: args.quiet,
            verbose: args.verbose,
            max_buffered_chunks: args.max_buffered_chunks,
        }
    }
}

fn parse_decimal_size(num: &str, multiplier: u128) -> Option<u128> {
    let (whole, fraction) = num.split_once('.').unwrap_or((num, ""));
    if whole.starts_with('-') || fraction.contains('.') {
        return None;
    }

    let whole = if whole.is_empty() {
        0
    } else {
        whole.parse::<u128>().ok()?
    };

    let whole = whole.checked_mul(multiplier)?;
    if fraction.is_empty() {
        return Some(whole);
    }

    let fraction_value = fraction.parse::<u128>().ok()?;
    let scale = 10_u128.checked_pow(u32::try_from(fraction.len()).ok()?)?;
    let fraction = fraction_value.checked_mul(multiplier)?.checked_div(scale)?;

    whole.checked_add(fraction)
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
        .ok_or_else(|| format!("Invalid size format: {s}"))?;

    let multiplier: u128 = match suffix.trim() {
        "B" => 1,
        "KB" | "K" => 1_000,
        "KIB" => 1_024,
        "MB" | "M" => 1_000_000,
        "MIB" => 1_048_576,
        "GB" | "G" => 1_000_000_000,
        "GIB" => 1_073_741_824,
        "TB" | "T" => 1_000_000_000_000,
        "TIB" => 1_099_511_627_776,
        _ => return Err(format!("Unknown size suffix: {suffix}")),
    };

    let value = parse_decimal_size(num_str.trim(), multiplier)
        .ok_or_else(|| format!("Invalid number: {num_str}"))?;

    usize::try_from(value).map_err(|_| format!("Size is too large: {s}"))
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
