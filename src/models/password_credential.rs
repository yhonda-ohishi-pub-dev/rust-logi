use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct PasswordCredential {
    pub id: uuid::Uuid,
    pub app_user_id: uuid::Uuid,
    pub organization_id: uuid::Uuid,
    pub username: String,
    pub password_hash: String,
    pub enabled: bool,
}
