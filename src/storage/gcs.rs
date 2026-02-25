use google_cloud_storage::{
    client::{Client, ClientConfig},
    http::objects::{
        delete::DeleteObjectRequest,
        download::Range,
        get::GetObjectRequest,
        upload::{Media, UploadObjectRequest, UploadType},
    },
};

use crate::error::{AppError, AppResult};

use super::{ObjectInfo, RestoreStatus, StorageBackend};

pub struct GcsBackend {
    client: Client,
    bucket: String,
}

impl GcsBackend {
    pub async fn new(bucket: String) -> AppResult<Self> {
        let config = ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| AppError::Storage(format!("GCS auth failed: {}", e)))?;
        let client = Client::new(config);
        Ok(Self { client, bucket })
    }
}

#[tonic::async_trait]
impl StorageBackend for GcsBackend {
    async fn upload(&self, key: &str, data: &[u8], content_type: &str) -> AppResult<String> {
        let mut media = Media::new(key.to_string());
        media.content_type = std::borrow::Cow::Owned(content_type.to_string());
        let upload_type = UploadType::Simple(media);

        self.client
            .upload_object(
                &UploadObjectRequest {
                    bucket: self.bucket.clone(),
                    ..Default::default()
                },
                data.to_vec(),
                &upload_type,
            )
            .await
            .map_err(|e| AppError::Storage(format!("GCS upload failed: {}", e)))?;

        tracing::info!("GCS upload: bucket={}, key={}", self.bucket, key);
        Ok(format!("gs://{}/{}", self.bucket, key))
    }

    async fn download(&self, key: &str) -> AppResult<Vec<u8>> {
        let data = self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: key.to_string(),
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
            .map_err(|e| AppError::Storage(format!("GCS download failed: {}", e)))?;

        tracing::info!(
            "GCS download: bucket={}, key={}, size={}",
            self.bucket,
            key,
            data.len()
        );
        Ok(data)
    }

    async fn delete(&self, key: &str) -> AppResult<()> {
        self.client
            .delete_object(&DeleteObjectRequest {
                bucket: self.bucket.clone(),
                object: key.to_string(),
                ..Default::default()
            })
            .await
            .map_err(|e| AppError::Storage(format!("GCS delete failed: {}", e)))?;

        tracing::info!("GCS delete: bucket={}, key={}", self.bucket, key);
        Ok(())
    }

    async fn get_object_info(&self, key: &str) -> AppResult<ObjectInfo> {
        let obj = self
            .client
            .get_object(&GetObjectRequest {
                bucket: self.bucket.clone(),
                object: key.to_string(),
                ..Default::default()
            })
            .await
            .map_err(|e| AppError::Storage(format!("GCS get object failed: {}", e)))?;

        Ok(ObjectInfo {
            storage_class: obj.storage_class.clone(),
            restore_status: RestoreStatus::NotNeeded,
            content_type: obj.content_type,
            size: Some(obj.size),
        })
    }

    async fn rewrite_to_standard(&self, key: &str) -> AppResult<()> {
        tracing::info!(
            "GCS rewrite_to_standard called (no-op with Autoclass): bucket={}, key={}",
            self.bucket,
            key
        );
        Ok(())
    }

    fn bucket(&self) -> &str {
        &self.bucket
    }
}
