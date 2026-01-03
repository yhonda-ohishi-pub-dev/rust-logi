use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FileModel {
    pub uuid: String,
    pub filename: String,
    #[sqlx(rename = "type")]
    pub file_type: String,
    pub created: String,
    pub deleted: Option<String>,
    pub blob: Option<String>,
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
        }
    }
}
