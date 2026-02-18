use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct ApiUser {
    pub id: String,
    pub organization_id: String,
    pub username: String,
    pub password_hash: String,
    pub enabled: bool,
}
