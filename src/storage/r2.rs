use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::Region;

use crate::error::{AppError, AppResult};

use super::{ObjectInfo, RestoreStatus, StorageBackend};

pub struct R2Backend {
    bucket: Box<Bucket>,
    bucket_name: String,
}

impl R2Backend {
    pub fn new(
        bucket_name: String,
        account_id: String,
        access_key: String,
        secret_key: String,
    ) -> AppResult<Self> {
        let region = Region::Custom {
            region: "auto".to_string(),
            endpoint: format!("https://{}.r2.cloudflarestorage.com", account_id),
        };

        let credentials = Credentials::new(
            Some(&access_key),
            Some(&secret_key),
            None, // security token
            None, // session token
            None, // profile
        )
        .map_err(|e| AppError::Storage(format!("R2 credentials error: {}", e)))?;

        let bucket = Bucket::new(&bucket_name, region, credentials)
            .map_err(|e| AppError::Storage(format!("R2 bucket error: {}", e)))?;

        Ok(Self {
            bucket,
            bucket_name,
        })
    }
}

#[tonic::async_trait]
impl StorageBackend for R2Backend {
    async fn upload(&self, key: &str, data: &[u8], content_type: &str) -> AppResult<String> {
        self.bucket
            .put_object_with_content_type(key, data, content_type)
            .await
            .map_err(|e| AppError::Storage(format!("R2 upload failed: {}", e)))?;

        tracing::info!("R2 upload: bucket={}, key={}", self.bucket_name, key);
        Ok(format!("r2://{}/{}", self.bucket_name, key))
    }

    async fn download(&self, key: &str) -> AppResult<Vec<u8>> {
        let response = self
            .bucket
            .get_object(key)
            .await
            .map_err(|e| AppError::Storage(format!("R2 download failed: {}", e)))?;

        tracing::info!(
            "R2 download: bucket={}, key={}, size={}",
            self.bucket_name,
            key,
            response.bytes().len()
        );
        Ok(response.bytes().to_vec())
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        self.bucket
            .delete_object(key)
            .await
            .map_err(|e| AppError::Storage(format!("R2 delete failed: {}", e)))?;

        tracing::info!("R2 delete: bucket={}, key={}", self.bucket_name, key);
        Ok(())
    }

    async fn get_object_info(&self, key: &str) -> AppResult<ObjectInfo> {
        let (head, _status) = self
            .bucket
            .head_object(key)
            .await
            .map_err(|e| AppError::Storage(format!("R2 head object failed: {}", e)))?;

        Ok(ObjectInfo {
            storage_class: Some("STANDARD".to_string()),
            restore_status: RestoreStatus::NotNeeded,
            content_type: head.content_type,
            size: head.content_length.map(|l| l as i64),
        })
    }

    async fn rewrite_to_standard(&self, key: &str) -> AppResult<()> {
        tracing::info!(
            "R2 rewrite_to_standard called (no-op): bucket={}, key={}",
            self.bucket_name,
            key
        );
        Ok(())
    }

    fn bucket(&self) -> &str {
        &self.bucket_name
    }
}
