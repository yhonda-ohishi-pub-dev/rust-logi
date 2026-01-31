use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct FlickrPhotoModel {
    pub id: String,
    pub secret: String,
    pub server: String,
}
