use crate::error::{Result, S3FcpError};

#[derive(Debug, Clone)]
pub struct S3Uri {
    pub bucket: String,
    pub key: String,
}

#[derive(Debug, Clone)]
pub struct HttpUri {
    pub url: String,
}

#[derive(Debug, Clone)]
pub enum DownloadUri {
    S3(S3Uri),
    Http(HttpUri),
}

impl DownloadUri {
    /// Parse a URI and select the downloader from its scheme.
    ///
    /// # Errors
    /// Returns an error if the URI scheme is unsupported or the URI is invalid.
    pub fn parse(uri: &str) -> Result<Self> {
        if uri.starts_with("s3://") {
            return Ok(Self::S3(S3Uri::parse(uri)?));
        }

        if uri.starts_with("http://") || uri.starts_with("https://") {
            return Ok(Self::Http(HttpUri::parse(uri)?));
        }

        Err(S3FcpError::InvalidUri(
            "URI must start with s3://, http://, or https://".to_string(),
        ))
    }
}

impl HttpUri {
    /// Parse and validate an HTTP/HTTPS URL.
    ///
    /// # Errors
    /// Returns an error if the URL does not start with `http://` or `https://`.
    pub fn parse(url: &str) -> Result<Self> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(S3FcpError::InvalidUri(
                "URL must start with http:// or https://".to_string(),
            ));
        }
        Ok(Self {
            url: url.to_string(),
        })
    }
}

impl S3Uri {
    /// Parse and validate an `s3://bucket/key` URI.
    ///
    /// # Errors
    /// Returns an error if the URI is missing the `s3://` prefix, bucket, or key.
    pub fn parse(uri: &str) -> Result<Self> {
        // Check for s3:// prefix
        if !uri.starts_with("s3://") {
            return Err(S3FcpError::InvalidUri(
                "URI must start with s3://".to_string(),
            ));
        }

        // Remove s3:// prefix
        let without_prefix = &uri[5..];

        // Split into bucket and key
        let parts: Vec<&str> = without_prefix.splitn(2, '/').collect();

        if parts.is_empty() || parts[0].is_empty() {
            return Err(S3FcpError::InvalidUri("Bucket name is missing".to_string()));
        }

        let bucket = parts[0].to_string();

        // Key is optional (can be empty for bucket root, though S3 doesn't allow downloading buckets)
        let key = if parts.len() > 1 {
            parts[1].to_string()
        } else {
            return Err(S3FcpError::InvalidUri("Object key is missing".to_string()));
        };

        if key.is_empty() {
            return Err(S3FcpError::InvalidUri(
                "Object key cannot be empty".to_string(),
            ));
        }

        Ok(Self { bucket, key })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_uri() {
        let uri = S3Uri::parse("s3://my-bucket/path/to/object.txt").unwrap();
        assert_eq!(uri.bucket, "my-bucket");
        assert_eq!(uri.key, "path/to/object.txt");
    }

    #[test]
    fn test_uri_with_trailing_slash() {
        let uri = S3Uri::parse("s3://my-bucket/folder/").unwrap();
        assert_eq!(uri.bucket, "my-bucket");
        assert_eq!(uri.key, "folder/");
    }

    #[test]
    fn test_invalid_uri_no_prefix() {
        let result = S3Uri::parse("my-bucket/key");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_uri_no_key() {
        let result = S3Uri::parse("s3://my-bucket");
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_uri_empty_key() {
        let result = S3Uri::parse("s3://my-bucket/");
        assert!(result.is_err());
    }

    #[test]
    fn test_download_uri_s3() {
        let uri = DownloadUri::parse("s3://my-bucket/path/to/object.txt").unwrap();
        match uri {
            DownloadUri::S3(uri) => {
                assert_eq!(uri.bucket, "my-bucket");
                assert_eq!(uri.key, "path/to/object.txt");
            }
            DownloadUri::Http(_) => panic!("expected S3 URI"),
        }
    }

    #[test]
    fn test_download_uri_http() {
        let uri = DownloadUri::parse("https://example.com/file.txt").unwrap();
        match uri {
            DownloadUri::Http(uri) => assert_eq!(uri.url, "https://example.com/file.txt"),
            DownloadUri::S3(_) => panic!("expected HTTP URI"),
        }
    }

    #[test]
    fn test_download_uri_invalid_scheme() {
        let result = DownloadUri::parse("ftp://example.com/file.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_http_uri_https() {
        let uri = HttpUri::parse("https://example.com/file.txt").unwrap();
        assert_eq!(uri.url, "https://example.com/file.txt");
    }

    #[test]
    fn test_http_uri_http() {
        let uri = HttpUri::parse("http://example.com/file.txt").unwrap();
        assert_eq!(uri.url, "http://example.com/file.txt");
    }

    #[test]
    fn test_http_uri_invalid_scheme() {
        let result = HttpUri::parse("ftp://example.com/file.txt");
        assert!(result.is_err());
    }
}
