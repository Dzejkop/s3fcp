#[derive(Debug, thiserror::Error)]
pub enum S3FcpError {
    #[error("Invalid URI: {0}")]
    InvalidUri(String),

    #[error("S3 operation failed: {0}")]
    S3Error(String),

    #[error("HTTP error: {0}")]
    HttpError(String),

    #[error("Request error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}

pub type Result<T> = std::result::Result<T, S3FcpError>;
