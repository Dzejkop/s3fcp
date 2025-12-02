#[derive(Debug, thiserror::Error)]
pub enum S3FcpError {
    #[error("Invalid S3 URI: {0}")]
    InvalidUri(String),

    #[error("S3 operation failed: {0}")]
    S3Error(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, S3FcpError>;
