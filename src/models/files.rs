use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FileModel {
    pub uuid: String,
    pub filename: String,
    pub file_type: String,
    pub created: String,
    pub deleted: Option<String>,
    pub blob: Option<String>,
    // S3 storage fields
    pub s3_key: Option<String>,
    pub storage_class: Option<String>,
    pub last_accessed_at: Option<String>,
    // Access tracking fields
    pub access_count_weekly: Option<i32>,
    pub access_count_total: Option<i32>,
    pub promoted_to_standard_at: Option<String>,
}

/// Result of recording file access
#[derive(Debug, Clone, FromRow)]
pub struct FileAccessResult {
    pub weekly_count: i32,
    pub total_count: i32,
    pub recent_7day_count: i32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FilesAppendModel {
    pub file_uuid: String,
    pub appendname: String,
    pub appendtype: String,
    #[sqlx(rename = "type")]
    pub file_type: String,
    pub page: Option<i32>,
    pub created: String,
    pub deleted: Option<String>,
}

impl FileModel {
    pub fn new(uuid: String, filename: String, file_type: String, blob: Option<String>) -> Self {
        Self {
            uuid,
            filename,
            file_type,
            created: chrono::Utc::now().to_rfc3339(),
            deleted: None,
            blob,
            s3_key: None,
            storage_class: None,
            last_accessed_at: None,
            access_count_weekly: None,
            access_count_total: None,
            promoted_to_standard_at: None,
        }
    }

    pub fn new_with_s3(uuid: String, filename: String, file_type: String, s3_key: String) -> Self {
        Self {
            uuid,
            filename,
            file_type,
            created: chrono::Utc::now().to_rfc3339(),
            deleted: None,
            blob: None,
            s3_key: Some(s3_key),
            storage_class: Some("STANDARD".to_string()),
            last_accessed_at: Some(chrono::Utc::now().to_rfc3339()),
            access_count_weekly: Some(0),
            access_count_total: Some(0),
            promoted_to_standard_at: None,
        }
    }
}
