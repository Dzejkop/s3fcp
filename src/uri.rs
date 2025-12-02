use crate::error::{Result, S3FcpError};

#[derive(Debug, Clone)]
pub struct S3Uri {
    pub bucket: String,
    pub key: String,
}

impl S3Uri {
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

        Ok(S3Uri { bucket, key })
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
}
