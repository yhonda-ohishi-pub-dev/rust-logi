// Storage abstraction for GCS and R2 backends

pub mod gcs;
pub mod r2;

pub use gcs::GcsBackend;
pub use r2::R2Backend;

// Backward compatibility alias
pub type GcsClient = GcsBackend;

use crate::error::AppResult;

/// オブジェクトの復元状態
#[derive(Debug, Clone, PartialEq)]
pub enum RestoreStatus {
    /// 復元不要（GCS Autoclass / R2 ともに即座にアクセス可能）
    NotNeeded,
    /// 復元進行中
    InProgress,
    /// 復元完了
    Completed,
    /// 復元が必要
    Required,
}

/// バックエンド非依存のオブジェクトメタデータ
#[derive(Debug, Clone)]
pub struct ObjectInfo {
    pub storage_class: Option<String>,
    pub restore_status: RestoreStatus,
    pub content_type: Option<String>,
    pub size: Option<i64>,
}

/// ストレージバックエンド抽象化（GCS / R2 共通インタフェース）
#[tonic::async_trait]
pub trait StorageBackend: Send + Sync {
    /// ファイルをアップロード。ストレージパス文字列を返す
    async fn upload(&self, key: &str, data: &[u8], content_type: &str) -> AppResult<String>;

    /// ファイルをダウンロード
    async fn download(&self, key: &str) -> AppResult<Vec<u8>>;

    /// ファイルを削除
    async fn delete(&self, key: &str) -> AppResult<()>;

    /// オブジェクトメタデータを取得
    async fn get_object_info(&self, key: &str) -> AppResult<ObjectInfo>;

    /// STANDARD ストレージクラスへの書き換え（GCS Autoclass / R2 では no-op）
    async fn rewrite_to_standard(&self, key: &str) -> AppResult<()>;

    /// バケット名を取得
    fn bucket(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_restore_status() {
        assert_eq!(RestoreStatus::NotNeeded, RestoreStatus::NotNeeded);
    }
}
