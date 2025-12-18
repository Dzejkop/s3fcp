use bytes::Bytes;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub index: usize,
    pub start: u64,
    pub end: u64,
}

#[derive(Debug)]
pub struct DownloadedChunk {
    pub index: usize,
    pub data: Bytes,
}

/// Create chunks from content length and chunk size
pub fn create_chunks(content_length: u64, chunk_size: usize) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut start = 0u64;
    let mut index = 0;

    while start < content_length {
        let end = (start + chunk_size as u64 - 1).min(content_length - 1);
        chunks.push(Chunk { index, start, end });
        start = end + 1;
        index += 1;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_chunks_empty_file() {
        let chunks = create_chunks(0, 100);
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn test_create_chunks_exact_multiple() {
        let chunks = create_chunks(1000, 100);
        assert_eq!(chunks.len(), 10);
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, 99);
        assert_eq!(chunks[9].start, 900);
        assert_eq!(chunks[9].end, 999);
    }

    #[test]
    fn test_create_chunks_with_remainder() {
        let chunks = create_chunks(1050, 100);
        assert_eq!(chunks.len(), 11);
        assert_eq!(chunks[10].start, 1000);
        assert_eq!(chunks[10].end, 1049);
    }

    #[test]
    fn test_create_chunks_non_conforming_chunks() {
        let chunks = create_chunks(10, 7);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, 6);
        assert_eq!(chunks[1].start, 7);
        assert_eq!(chunks[1].end, 9);
    }

    #[test]
    fn test_create_chunks_single_chunk() {
        let chunks = create_chunks(50, 100);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, 49);
    }
}
