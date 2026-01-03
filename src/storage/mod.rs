// AWS S3 storage module with Glacier restore support

use aws_sdk_s3::{
    primitives::ByteStream,
    types::{GlacierJobParameters, RestoreRequest, Tier},
    Client,
};

use crate::error::{AppError, AppResult};

/// S3オブジェクトの復元状態
#[derive(Debug, Clone, PartialEq)]
pub enum RestoreStatus {
    /// 復元不要（Standard, Standard-IA等）
    NotNeeded,
    /// 復元進行中
    InProgress,
    /// 復元完了（一時的にアクセス可能）
    Completed,
    /// 復元が必要（Glacier/Deep Archive）
    Required,
}

/// S3オブジェクトの情報
#[derive(Debug, Clone)]
pub struct S3ObjectInfo {
    pub storage_class: Option<String>,
    pub restore_status: RestoreStatus,
    pub content_type: Option<String>,
    pub content_length: Option<i64>,
}

pub struct S3Client {
    client: Client,
    bucket: String,
}

impl S3Client {
    pub async fn new(bucket: String) -> Self {
        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = Client::new(&config);
        Self { client, bucket }
    }

    /// ファイルをS3にアップロード
    pub async fn upload(&self, key: &str, data: &[u8], content_type: &str) -> AppResult<String> {
        let body = ByteStream::from(data.to_vec());

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| AppError::Storage(format!("S3 upload failed: {}", e)))?;

        tracing::info!("S3 upload: bucket={}, key={}", self.bucket, key);
        Ok(format!("s3://{}/{}", self.bucket, key))
    }

    /// S3からファイルをダウンロード
    pub async fn download(&self, key: &str) -> AppResult<Vec<u8>> {
        let resp = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::Storage(format!("S3 download failed: {}", e)))?;

        let data = resp
            .body
            .collect()
            .await
            .map_err(|e| AppError::Storage(format!("Failed to read S3 body: {}", e)))?
            .into_bytes()
            .to_vec();

        tracing::info!("S3 download: bucket={}, key={}, size={}", self.bucket, key, data.len());
        Ok(data)
    }

    /// S3オブジェクトを削除
    pub async fn delete(&self, key: &str) -> AppResult<()> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::Storage(format!("S3 delete failed: {}", e)))?;

        tracing::info!("S3 delete: bucket={}, key={}", self.bucket, key);
        Ok(())
    }

    /// S3オブジェクトの情報を取得
    pub async fn get_object_info(&self, key: &str) -> AppResult<S3ObjectInfo> {
        let resp = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|e| AppError::Storage(format!("S3 head object failed: {}", e)))?;

        let storage_class = resp.storage_class().map(|s| s.as_str().to_string());
        let restore_status = self.parse_restore_status(&storage_class, resp.restore());

        Ok(S3ObjectInfo {
            storage_class,
            restore_status,
            content_type: resp.content_type().map(|s| s.to_string()),
            content_length: resp.content_length(),
        })
    }

    /// Glacierからの復元をリクエスト
    /// days: 復元後のアクセス可能日数
    /// tier: 復元速度（Expedited: 1-5分, Standard: 3-5時間, Bulk: 5-12時間）
    pub async fn request_restore(&self, key: &str, days: i32, tier: Tier) -> AppResult<()> {
        let tier_str = format!("{:?}", tier);
        let glacier_params = GlacierJobParameters::builder().tier(tier).build()?;

        let restore_request = RestoreRequest::builder()
            .days(days)
            .glacier_job_parameters(glacier_params)
            .build();

        self.client
            .restore_object()
            .bucket(&self.bucket)
            .key(key)
            .restore_request(restore_request)
            .send()
            .await
            .map_err(|e| {
                // 既に復元リクエスト済みの場合はエラーにしない
                let error_str = format!("{}", e);
                if error_str.contains("RestoreAlreadyInProgress") {
                    tracing::info!("Restore already in progress: key={}", key);
                    return AppError::RestoreInProgress;
                }
                AppError::Storage(format!("S3 restore request failed: {}", e))
            })?;

        tracing::info!(
            "S3 restore requested: bucket={}, key={}, days={}, tier={}",
            self.bucket,
            key,
            days,
            tier_str
        );
        Ok(())
    }

    /// オブジェクトをStandardストレージクラスにコピー（IA/Glacier復元後）
    pub async fn copy_to_standard(&self, key: &str) -> AppResult<()> {
        let copy_source = format!("{}/{}", self.bucket, key);

        self.client
            .copy_object()
            .bucket(&self.bucket)
            .key(key)
            .copy_source(&copy_source)
            .storage_class(aws_sdk_s3::types::StorageClass::Standard)
            .metadata_directive(aws_sdk_s3::types::MetadataDirective::Copy)
            .send()
            .await
            .map_err(|e| AppError::Storage(format!("S3 copy to standard failed: {}", e)))?;

        tracing::info!("S3 copy to standard: bucket={}, key={}", self.bucket, key);
        Ok(())
    }

    /// 復元状態を解析
    fn parse_restore_status(
        &self,
        storage_class: &Option<String>,
        restore_header: Option<&str>,
    ) -> RestoreStatus {
        // Standard, Standard-IA, Intelligent-Tiering等は復元不要
        let needs_restore = matches!(
            storage_class.as_deref(),
            Some("GLACIER") | Some("DEEP_ARCHIVE") | Some("GLACIER_IR")
        );

        if !needs_restore {
            return RestoreStatus::NotNeeded;
        }

        // x-amz-restore ヘッダーを解析
        // 形式: ongoing-request="true" または ongoing-request="false", expiry-date="..."
        match restore_header {
            None => RestoreStatus::Required,
            Some(header) => {
                if header.contains("ongoing-request=\"true\"") {
                    RestoreStatus::InProgress
                } else if header.contains("ongoing-request=\"false\"") {
                    RestoreStatus::Completed
                } else {
                    RestoreStatus::Required
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_restore_status_standard() {
        let client = S3Client {
            client: unsafe { std::mem::zeroed() }, // テスト用のダミー
            bucket: "test".to_string(),
        };

        let status = client.parse_restore_status(&Some("STANDARD".to_string()), None);
        assert_eq!(status, RestoreStatus::NotNeeded);

        let status = client.parse_restore_status(&Some("STANDARD_IA".to_string()), None);
        assert_eq!(status, RestoreStatus::NotNeeded);
    }

    #[test]
    fn test_parse_restore_status_glacier() {
        let client = S3Client {
            client: unsafe { std::mem::zeroed() },
            bucket: "test".to_string(),
        };

        let status = client.parse_restore_status(&Some("GLACIER".to_string()), None);
        assert_eq!(status, RestoreStatus::Required);

        let status = client.parse_restore_status(
            &Some("GLACIER".to_string()),
            Some("ongoing-request=\"true\""),
        );
        assert_eq!(status, RestoreStatus::InProgress);

        let status = client.parse_restore_status(
            &Some("GLACIER".to_string()),
            Some("ongoing-request=\"false\", expiry-date=\"...\""),
        );
        assert_eq!(status, RestoreStatus::Completed);
    }
}
