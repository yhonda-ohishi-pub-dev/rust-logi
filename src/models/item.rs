use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct ItemModel {
    pub id: String,
    pub parent_id: Option<String>,
    pub owner_type: String,
    pub organization_id: Option<String>,
    pub user_id: Option<String>,
    pub name: String,
    pub barcode: Option<String>,
    pub category: Option<String>,
    pub description: Option<String>,
    pub image_url: Option<String>,
    pub url: Option<String>,
    pub item_type: String,
    pub quantity: i32,
    pub created_at: String,
    pub updated_at: String,
}
