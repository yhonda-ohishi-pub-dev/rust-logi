// Google Cloud Storage module with Autoclass support
// Autoclass automatically manages storage class transitions based on access patterns
// (similar to S3 Intelligent-Tiering)

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

/// GCSオブジェクトの復元状態
/// GCSではAutoclassを使用するため、復元は基本的に不要
#[derive(Debug, Clone, PartialEq)]
pub enum RestoreStatus {
    /// 復元不要（GCSではすべてのストレージクラスが即座にアクセス可能）
    NotNeeded,
    /// 復元進行中（GCSでは使用しない）
    InProgress,
    /// 復元完了（GCSでは使用しない）
    Completed,
    /// 復元が必要（GCSでは使用しない）
    Required,
}

/// GCSオブジェクトの情報
#[derive(Debug, Clone)]
pub struct GcsObjectInfo {
    pub storage_class: Option<String>,
    pub restore_status: RestoreStatus,
    pub content_type: Option<String>,
    pub size: Option<i64>,
}

pub struct GcsClient {
    client: Client,
    bucket: String,
}

impl GcsClient {
    pub async fn new(bucket: String) -> AppResult<Self> {
        let config = ClientConfig::default()
            .with_auth()
            .await
            .map_err(|e| AppError::Storage(format!("GCS auth failed: {}", e)))?;
        let client = Client::new(config);
        Ok(Self { client, bucket })
    }

    /// ファイルをGCSにアップロード
    /// Autoclassが有効なバケットでは、ストレージクラスは自動的に管理される
    pub async fn upload(&self, key: &str, data: &[u8], _content_type: &str) -> AppResult<String> {
        let upload_type = UploadType::Simple(Media::new(key.to_string()));

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

    /// GCSからファイルをダウンロード
    /// GCSではすべてのストレージクラスが即座にアクセス可能
    pub async fn download(&self, key: &str) -> AppResult<Vec<u8>> {
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

    /// GCSオブジェクトを削除
    pub async fn delete(&self, key: &str) -> AppResult<()> {
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

    /// GCSオブジェクトの情報を取得
    pub async fn get_object_info(&self, key: &str) -> AppResult<GcsObjectInfo> {
        let obj = self
            .client
            .get_object(&GetObjectRequest {
                bucket: self.bucket.clone(),
                object: key.to_string(),
                ..Default::default()
            })
            .await
            .map_err(|e| AppError::Storage(format!("GCS get object failed: {}", e)))?;

        Ok(GcsObjectInfo {
            storage_class: obj.storage_class.clone(),
            restore_status: RestoreStatus::NotNeeded, // GCSでは常にアクセス可能
            content_type: obj.content_type,
            size: Some(obj.size),
        })
    }

    /// Autoclassが有効なバケットでは、ストレージクラスの手動変更は不要
    /// この関数は後方互換性のために残すが、実質的には何もしない
    pub async fn rewrite_to_standard(&self, key: &str) -> AppResult<()> {
        // Autoclassが有効な場合、ストレージクラスは自動的に管理される
        // 手動でのrewriteは不要だが、ログだけ残す
        tracing::info!(
            "GCS rewrite_to_standard called (no-op with Autoclass): bucket={}, key={}",
            self.bucket,
            key
        );
        Ok(())
    }

    /// バケット名を取得
    pub fn bucket(&self) -> &str {
        &self.bucket
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restore_status() {
        // GCSではすべてのストレージクラスが即座にアクセス可能
        assert_eq!(RestoreStatus::NotNeeded, RestoreStatus::NotNeeded);
    }
}
