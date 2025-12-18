# s3fcp - Fast File Downloader

A high-performance Rust CLI tool for downloading files from S3 and HTTP/HTTPS with multi-part concurrent downloads and ordered streaming to stdout.

## Features

- **Multi-part Downloads**: Split large files into chunks and download concurrently
- **S3 and HTTP Support**: Download from S3 buckets or any HTTP/HTTPS URL
- **Ordered Streaming**: Maintains correct byte order while streaming to stdout
- **Configurable Concurrency**: Control the number of parallel download workers
- **Configurable Chunk Size**: Adjust chunk size for optimal performance
- **Version ID Support**: Download specific versions of S3 objects
- **Range Request Support**: Uses HTTP Range requests when supported, falls back to single-stream otherwise
- **Progress Tracking**: Real-time progress bar on stderr (can be silenced)
- **Automatic Retries**: Exponential backoff retry logic for transient failures
- **Memory Efficient**: Bounded memory usage regardless of file size

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
./target/release/s3fcp --help
```

## Usage

### S3 Downloads

```bash
# Download and stream to stdout
s3fcp s3 s3://bucket/key

# Redirect to file
s3fcp s3 s3://bucket/key > output.bin

# Download specific version
s3fcp s3 s3://bucket/key --version-id v123

# Increase concurrency
s3fcp s3 s3://bucket/key -c 16

# Use larger chunks (human-readable sizes)
s3fcp s3 s3://bucket/key --chunk-size 16MB
```

### HTTP/HTTPS Downloads

```bash
# Download from HTTP URL
s3fcp http https://example.com/file.bin > file.bin

# With custom concurrency and chunk size
s3fcp http https://example.com/large.iso -c 16 --chunk-size 16MB

# Quiet mode
s3fcp http https://example.com/data.json -q | jq '.field'
```

## CLI Options

```
Usage: s3fcp <COMMAND>

Commands:
  s3    Download from S3
  http  Download from HTTP/HTTPS URL
  help  Print this message or the help of the given subcommand(s)
```

### S3 Subcommand

```
Usage: s3fcp s3 [OPTIONS] <URI>

Arguments:
  <URI>  S3 URI in the format s3://bucket/key

Options:
      --version-id <VERSION_ID>    S3 object version ID for versioned objects
  -c, --concurrency <CONCURRENCY>  Number of concurrent download workers [default: 10]
      --chunk-size <CHUNK_SIZE>    Chunk size [default: 8MB]
  -q, --quiet                      Quiet mode - suppress progress output
  -h, --help                       Print help
```

### HTTP Subcommand

```
Usage: s3fcp http [OPTIONS] <URL>

Arguments:
  <URL>  HTTP/HTTPS URL to download

Options:
  -c, --concurrency <CONCURRENCY>  Number of concurrent download workers [default: 10]
      --chunk-size <CHUNK_SIZE>    Chunk size [default: 8MB]
  -q, --quiet                      Quiet mode - suppress progress output
  -h, --help                       Print help
```

Supported chunk size formats:
- Plain numbers: `8388608` (bytes)
- Decimal: `8MB`, `1GB`, `1TB` (powers of 1000)
- Binary: `8MiB`, `1GiB`, `1TiB` (powers of 1024)

## Architecture

s3fcp uses a 3-stage pipeline architecture:

### Stage 1: Queue
- Creates download jobs for each chunk
- Uses bounded channel for natural backpressure
- Prevents overwhelming downstream workers

### Stage 2: Download Workers
- Worker pool (size = concurrency)
- Downloads chunks using Range GET requests
- Automatic retry with exponential backoff
- Updates progress tracker

### Stage 3: Ordered Output
- Receives chunks (potentially out of order)
- Buffers chunks in BTreeMap
- Streams to stdout in correct order
- Memory-bounded buffering

### HTTP Range Support

For HTTP downloads, s3fcp checks if the server supports Range requests via the `Accept-Ranges` header. If supported, it uses chunked parallel downloads. Otherwise, it falls back to a single-stream download.

## Performance

Memory usage is bounded by:
```
Max Memory ≈ 2 × concurrency × chunk_size
```

With defaults (concurrency=10, chunk_size=8MB):
```
Max Memory ≈ 160MB
```

This holds regardless of the file size.

## AWS Credentials

For S3 downloads, s3fcp uses the standard AWS credential chain:
- Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
- AWS credentials file (`~/.aws/credentials`)
- IAM roles (when running on EC2/ECS)

## Examples

Download a 1GB file from S3 with 16 concurrent workers:
```bash
s3fcp s3 s3://my-bucket/large-file.bin -c 16 > large-file.bin
```

Download from HTTP and pipe to another command:
```bash
s3fcp http https://example.com/data.gz -q | gunzip | grep "pattern"
```

Quiet download for scripts:
```bash
s3fcp s3 s3://my-bucket/data.json -q | jq '.field'
```

## Development

Run tests:
```bash
cargo test
```

Build for release:
```bash
cargo build --release
```

Check code:
```bash
cargo check
cargo clippy
```

## License

MIT
