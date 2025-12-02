# s3fcp - Fast S3 Object Downloader

A high-performance Rust CLI tool for downloading objects from S3 with multi-part concurrent downloads and ordered streaming to stdout.

## Features

- **Multi-part Downloads**: Split large files into chunks and download concurrently
- **Ordered Streaming**: Maintains correct byte order while streaming to stdout
- **Configurable Concurrency**: Control the number of parallel download workers
- **Configurable Chunk Size**: Adjust chunk size for optimal performance
- **Version ID Support**: Download specific versions of S3 objects
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

Basic usage:

```bash
# Download and stream to stdout
s3fcp s3://bucket/key

# Redirect to file
s3fcp s3://bucket/key > output.bin

# Download specific version
s3fcp s3://bucket/key --version-id v123

# Increase concurrency
s3fcp s3://bucket/key -c 16

# Use larger chunks (human-readable sizes)
s3fcp s3://bucket/key --chunk-size 16MB
s3fcp s3://bucket/key --chunk-size 16MiB
s3fcp s3://bucket/key --chunk-size 1GB

# Quiet mode (no progress bar)
s3fcp s3://bucket/key -q
```

## CLI Options

```
Usage: s3fcp [OPTIONS] <S3_URI>

Arguments:
  <S3_URI>  S3 URI in the format s3://bucket/key

Options:
  --version-id <VERSION_ID>      S3 object version ID for versioned objects
  -c, --concurrency <CONCURRENCY>  Number of concurrent download workers [default: 10]
  --chunk-size <CHUNK_SIZE>      Chunk size (supports human-readable sizes: 8MB, 16MiB, 1GB, etc.) [default: 8MB]
  -q, --quiet                    Quiet mode - suppress progress output
  -h, --help                     Print help
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
- Downloads chunks using S3 range GET requests
- Automatic retry with exponential backoff
- Updates progress tracker

### Stage 3: Ordered Output
- Receives chunks (potentially out of order)
- Buffers chunks in BTreeMap
- Streams to stdout in correct order
- Memory-bounded buffering

## Performance

Memory usage is bounded by:
```
Max Memory ≈ 2 × concurrency × chunk_size
```

With defaults (concurrency=10, chunk_size=8MB):
```
Max Memory ≈ 160MB
```

This holds regardless of the S3 object size.

## AWS Credentials

s3fcp uses the standard AWS credential chain:
- Environment variables (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`)
- AWS credentials file (`~/.aws/credentials`)
- IAM roles (when running on EC2/ECS)

## Examples

Download a 1GB file with 16 concurrent workers:
```bash
s3fcp s3://my-bucket/large-file.bin -c 16 > large-file.bin
```

Pipe directly to another command:
```bash
s3fcp s3://my-bucket/data.gz | gunzip | grep "pattern"
```

Quiet download for scripts:
```bash
s3fcp s3://my-bucket/data.json -q | jq '.field'
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
