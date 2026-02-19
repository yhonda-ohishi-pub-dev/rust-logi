use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct OrganizationModel {
    pub id: uuid::Uuid,
    pub name: String,
    pub slug: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
