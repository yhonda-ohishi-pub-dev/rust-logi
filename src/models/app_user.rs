use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct AppUser {
    pub id: uuid::Uuid,
    pub email: Option<String>,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub is_superadmin: bool,
}
