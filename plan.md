# S3 Fast Copy (s3fcp) Implementation Plan

## Overview

Build a Rust CLI tool for high-performance S3 downloads with multi-part downloading, ordered streaming to stdout, and configurable concurrency.

## Dependencies (Cargo.toml)

- `aws-config = "1.5"` - AWS credential chain
- `aws-sdk-s3 = "1.55"` - S3 operations
- `tokio = { version = "1.40", features = ["full"] }` - Async runtime
- `backon = "1.6.0"` - Retry logic
- `clap = { version = "4.5", features = ["derive"] }` - CLI parsing
- `indicatif = "0.17"` - Progress bars
- `anyhow = "1.0"`, `thiserror = "1.0"` - Error handling
- `bytes = "1.7"` - Efficient byte buffers
- `futures = "0.3"` - Async utilities

## CLI Interface

```bash
s3fcp s3://bucket/key                    # Basic usage
s3fcp s3://bucket/key --version-id <id>  # Specific version
s3fcp s3://bucket/key -c 16              # 16 concurrent downloads
s3fcp s3://bucket/key --chunk-size 16777216  # 16MB chunks
s3fcp s3://bucket/key -q                 # Quiet mode (no progress)
s3fcp s3://bucket/key > output.bin       # Redirect to file
```

## Architecture: 3-Stage Pipeline

### Stage 1: Queue Up Download Jobs

- Use bounded `tokio::mpsc::channel(concurrency)` for natural backpressure
- Producer iterates through chunks and sends to channel
- If channel full, producer waits (prevents overwhelming workers)

### Stage 2: Execute Downloads Async

- Spawn worker pool (size = concurrency)
- Each worker pulls chunks from queue
- Downloads using S3 `GetObject` with byte ranges
- Wraps operations in retry logic with exponential backoff
- Sends `(index, data)` to output stage
- Updates progress tracker

### Stage 3: Collect & Order Output

- **Critical for correctness:** Must maintain chunk ordering
- Use `BTreeMap<index, Bytes>` to buffer out-of-order chunks
- Track `next_expected_index` (starts at 0)
- When next sequential chunk arrives:
  - Write to stdout immediately
  - Drain any subsequent sequential chunks
  - Increment next_expected_index
- Memory-bounded: only buffers chunks waiting for earlier chunks

## Key Implementation Details

### Chunk Creation

```rust
struct Chunk {
    index: usize,           // For ordering
    start: u64,             // Byte range start (inclusive)
    end: u64,               // Byte range end (inclusive)
    version_id: Option<String>,
}
```

- Get content_length from HEAD request
- Split into chunks of configured size
- Last chunk may be smaller

### Retry Logic

Use backon to facilitate retry logic

The blow are not strictly required - only implement them insofar as backon allows for it.

- Max attempts: 3 (configurable)
- Initial delay: 100ms
- Max delay: 5s
- Multiplier: 2.0 (exponential backoff)
- Retry on transient S3 errors, network failures

### Progress Tracking

- Uses `indicatif::ProgressBar`
- Shows: bytes downloaded, percentage, speed
- Output to stderr (unless `--quiet`)
- Thread-safe updates from multiple workers

### Orchestration Flow

1. Parse S3 URI (`s3://bucket/key`)
2. Initialize AWS S3 client
3. HEAD request to get content_length
5. Create chunks based on content_length and chunk_size
6. Setup progress tracker
7. Create channels for stages
8. Spawn Stage 1 (queue task)
9. Spawn Stage 2 (concurrency worker tasks)
10. Spawn Stage 3 (output task)
11. Await all tasks, handle errors

## Edge Cases

### Out-of-Order Completion

- Stage 3 buffers in BTreeMap
- Only writes when sequential chunks available
- Worst case buffer: (concurrency - 1) chunks

### Network Errors

- Each S3 operation wrapped in retry logic
- Exponential backoff between attempts
- Fail gracefully with clear error after max retries

### Memory Management

- Use `Bytes` for zero-copy operations
- Stream to stdout immediately when ordered
- Don't buffer entire file
- Max memory: ~2 *concurrency* chunk_size

## Testing Strategy

- Unit tests for chunk creation
- Unit tests for URI parsing
- Integration test with small S3 object
- Test ordering with simulated out-of-order chunks
- Test retry logic with simulated failures
