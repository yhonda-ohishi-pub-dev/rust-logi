use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CamFileModel {
    pub name: String,
    pub date: String,
    pub hour: String,
    #[sqlx(rename = "type")]
    pub file_type: String,
    pub cam: String,
    pub flickr_id: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CamFileExeModel {
    pub name: String,
    pub cam: String,
    pub stage: i32,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CamFileExeStageModel {
    pub stage: i32,
    pub name: String,
}
