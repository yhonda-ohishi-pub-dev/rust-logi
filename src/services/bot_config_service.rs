use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::db::organization::set_current_organization;
use crate::middleware::AuthenticatedUser;
use crate::proto::bot_config::bot_config_service_server::BotConfigService;
use crate::proto::bot_config::{
    BotConfigResponse, BotConfigWithSecretsResponse, DeleteBotConfigRequest,
    DeleteBotConfigResponse, GetBotConfigRequest, ListBotConfigsRequest, ListBotConfigsResponse,
    UpsertBotConfigRequest,
};
use crate::services::lineworks_auth;

pub struct BotConfigServiceImpl {
    pool: PgPool,
    jwt_secret: String,
}

impl BotConfigServiceImpl {
    pub fn new(pool: PgPool, jwt_secret: String) -> Self {
        Self { pool, jwt_secret }
    }

    fn get_authenticated_user<T>(request: &Request<T>) -> Result<AuthenticatedUser, Status> {
        request
            .extensions()
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Authentication required"))
    }

    async fn verify_admin(&self, user_id: &str, org_id: &str) -> Result<(), Status> {
        let role: Option<(String,)> = sqlx::query_as(
            "SELECT role FROM user_organizations WHERE user_id = $1::uuid AND organization_id = $2::uuid",
        )
        .bind(user_id)
        .bind(org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match role {
            Some((r,)) if r == "admin" => Ok(()),
            Some(_) => Err(Status::permission_denied("Admin role required")),
            None => Err(Status::permission_denied("Not a member of this organization")),
        }
    }
}

#[tonic::async_trait]
impl BotConfigService for BotConfigServiceImpl {
    async fn list_configs(
        &self,
        request: Request<ListBotConfigsRequest>,
    ) -> Result<Response<ListBotConfigsResponse>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        self.verify_admin(&auth_user.user_id, &auth_user.org_id)
            .await?;

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Pool error: {}", e)))?;
        set_current_organization(&mut conn, &auth_user.org_id)
            .await
            .map_err(|e| Status::internal(format!("RLS error: {}", e)))?;

        let rows: Vec<(String, String, String, String, String, String, bool, String, String)> =
            sqlx::query_as(
                "SELECT id::text, provider, name, client_id, service_account, bot_id,
                        enabled, created_at::text, updated_at::text
                 FROM bot_configs
                 WHERE organization_id = $1::uuid
                 ORDER BY name",
            )
            .bind(&auth_user.org_id)
            .fetch_all(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let configs = rows
            .into_iter()
            .map(
                |(id, provider, name, client_id, service_account, bot_id, enabled, created_at, updated_at)| {
                    BotConfigResponse {
                        id,
                        provider,
                        name,
                        client_id,
                        has_client_secret: true,
                        service_account,
                        has_private_key: true,
                        bot_id,
                        enabled,
                        created_at,
                        updated_at,
                    }
                },
            )
            .collect();

        Ok(Response::new(ListBotConfigsResponse { configs }))
    }

    async fn get_config(
        &self,
        request: Request<GetBotConfigRequest>,
    ) -> Result<Response<BotConfigResponse>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        self.verify_admin(&auth_user.user_id, &auth_user.org_id)
            .await?;

        let req = request.into_inner();

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Pool error: {}", e)))?;
        set_current_organization(&mut conn, &auth_user.org_id)
            .await
            .map_err(|e| Status::internal(format!("RLS error: {}", e)))?;

        let row: Option<(String, String, String, String, String, String, bool, String, String)> =
            sqlx::query_as(
                "SELECT id::text, provider, name, client_id, service_account, bot_id,
                        enabled, created_at::text, updated_at::text
                 FROM bot_configs
                 WHERE id = $1::uuid AND organization_id = $2::uuid",
            )
            .bind(&req.id)
            .bind(&auth_user.org_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match row {
            Some((id, provider, name, client_id, service_account, bot_id, enabled, created_at, updated_at)) => {
                Ok(Response::new(BotConfigResponse {
                    id,
                    provider,
                    name,
                    client_id,
                    has_client_secret: true,
                    service_account,
                    has_private_key: true,
                    bot_id,
                    enabled,
                    created_at,
                    updated_at,
                }))
            }
            None => Err(Status::not_found("Bot config not found")),
        }
    }

    async fn upsert_config(
        &self,
        request: Request<UpsertBotConfigRequest>,
    ) -> Result<Response<BotConfigResponse>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        self.verify_admin(&auth_user.user_id, &auth_user.org_id)
            .await?;

        let req = request.into_inner();

        if req.name.is_empty() || req.client_id.is_empty() || req.bot_id.is_empty() || req.service_account.is_empty() {
            return Err(Status::invalid_argument(
                "name, client_id, service_account, and bot_id are required",
            ));
        }

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Pool error: {}", e)))?;
        set_current_organization(&mut conn, &auth_user.org_id)
            .await
            .map_err(|e| Status::internal(format!("RLS error: {}", e)))?;

        let config_id: String;

        if req.id.is_empty() {
            // Create new
            if req.client_secret.is_empty() || req.private_key.is_empty() {
                return Err(Status::invalid_argument(
                    "client_secret and private_key are required for new bot config",
                ));
            }

            let encrypted_secret =
                lineworks_auth::encrypt_secret(&req.client_secret, &self.jwt_secret)
                    .map_err(|e| Status::internal(format!("Encrypt error: {}", e)))?;
            let encrypted_key =
                lineworks_auth::encrypt_secret(&req.private_key, &self.jwt_secret)
                    .map_err(|e| Status::internal(format!("Encrypt error: {}", e)))?;

            let row: (String,) = sqlx::query_as(
                "INSERT INTO bot_configs
                 (organization_id, provider, name, client_id, client_secret_encrypted,
                  service_account, private_key_encrypted, bot_id, enabled)
                 VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8, $9)
                 RETURNING id::text",
            )
            .bind(&auth_user.org_id)
            .bind(if req.provider.is_empty() { "lineworks" } else { &req.provider })
            .bind(&req.name)
            .bind(&req.client_id)
            .bind(&encrypted_secret)
            .bind(&req.service_account)
            .bind(&encrypted_key)
            .bind(&req.bot_id)
            .bind(req.enabled)
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| {
                if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                    Status::already_exists("This bot_id is already configured")
                } else {
                    Status::internal(format!("Insert error: {}", e))
                }
            })?;

            config_id = row.0;
        } else {
            // Update existing
            config_id = req.id.clone();

            // Build dynamic update based on which secrets are provided
            if !req.client_secret.is_empty() && !req.private_key.is_empty() {
                let encrypted_secret =
                    lineworks_auth::encrypt_secret(&req.client_secret, &self.jwt_secret)
                        .map_err(|e| Status::internal(format!("Encrypt error: {}", e)))?;
                let encrypted_key =
                    lineworks_auth::encrypt_secret(&req.private_key, &self.jwt_secret)
                        .map_err(|e| Status::internal(format!("Encrypt error: {}", e)))?;

                sqlx::query(
                    "UPDATE bot_configs
                     SET provider = $1, name = $2, client_id = $3, client_secret_encrypted = $4,
                         service_account = $5, private_key_encrypted = $6, bot_id = $7,
                         enabled = $8, updated_at = NOW()
                     WHERE id = $9::uuid AND organization_id = $10::uuid",
                )
                .bind(if req.provider.is_empty() { "lineworks" } else { &req.provider })
                .bind(&req.name)
                .bind(&req.client_id)
                .bind(&encrypted_secret)
                .bind(&req.service_account)
                .bind(&encrypted_key)
                .bind(&req.bot_id)
                .bind(req.enabled)
                .bind(&req.id)
                .bind(&auth_user.org_id)
                .execute(&mut *conn)
                .await
                .map_err(|e| Status::internal(format!("Update error: {}", e)))?;
            } else {
                // Update without changing secrets
                sqlx::query(
                    "UPDATE bot_configs
                     SET provider = $1, name = $2, client_id = $3, service_account = $4,
                         bot_id = $5, enabled = $6, updated_at = NOW()
                     WHERE id = $7::uuid AND organization_id = $8::uuid",
                )
                .bind(if req.provider.is_empty() { "lineworks" } else { &req.provider })
                .bind(&req.name)
                .bind(&req.client_id)
                .bind(&req.service_account)
                .bind(&req.bot_id)
                .bind(req.enabled)
                .bind(&req.id)
                .bind(&auth_user.org_id)
                .execute(&mut *conn)
                .await
                .map_err(|e| Status::internal(format!("Update error: {}", e)))?;
            }
        }

        // Return updated config
        let (id, provider, name, client_id, service_account, bot_id, enabled, created_at, updated_at): (
            String, String, String, String, String, String, bool, String, String,
        ) = sqlx::query_as(
            "SELECT id::text, provider, name, client_id, service_account, bot_id,
                    enabled, created_at::text, updated_at::text
             FROM bot_configs
             WHERE id = $1::uuid AND organization_id = $2::uuid",
        )
        .bind(&config_id)
        .bind(&auth_user.org_id)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(BotConfigResponse {
            id,
            provider,
            name,
            client_id,
            has_client_secret: true,
            service_account,
            has_private_key: true,
            bot_id,
            enabled,
            created_at,
            updated_at,
        }))
    }

    async fn delete_config(
        &self,
        request: Request<DeleteBotConfigRequest>,
    ) -> Result<Response<DeleteBotConfigResponse>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        self.verify_admin(&auth_user.user_id, &auth_user.org_id)
            .await?;

        let req = request.into_inner();

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Pool error: {}", e)))?;
        set_current_organization(&mut conn, &auth_user.org_id)
            .await
            .map_err(|e| Status::internal(format!("RLS error: {}", e)))?;

        sqlx::query("DELETE FROM bot_configs WHERE id = $1::uuid AND organization_id = $2::uuid")
            .bind(&req.id)
            .bind(&auth_user.org_id)
            .execute(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Delete error: {}", e)))?;

        Ok(Response::new(DeleteBotConfigResponse {}))
    }

    async fn get_config_with_secrets(
        &self,
        request: Request<GetBotConfigRequest>,
    ) -> Result<Response<BotConfigWithSecretsResponse>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        self.verify_admin(&auth_user.user_id, &auth_user.org_id)
            .await?;

        let req = request.into_inner();

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Pool error: {}", e)))?;
        set_current_organization(&mut conn, &auth_user.org_id)
            .await
            .map_err(|e| Status::internal(format!("RLS error: {}", e)))?;

        let row: Option<(String, String, String, String, String, String, String, String)> =
            sqlx::query_as(
                "SELECT id::text, provider, name, client_id, client_secret_encrypted,
                        service_account, private_key_encrypted, bot_id
                 FROM bot_configs
                 WHERE id = $1::uuid AND organization_id = $2::uuid AND enabled = TRUE",
            )
            .bind(&req.id)
            .bind(&auth_user.org_id)
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match row {
            Some((id, provider, name, client_id, secret_enc, service_account, key_enc, bot_id)) => {
                let client_secret =
                    lineworks_auth::decrypt_secret(&secret_enc, &self.jwt_secret)
                        .map_err(|e| Status::internal(format!("Decrypt error: {}", e)))?;
                let private_key =
                    lineworks_auth::decrypt_secret(&key_enc, &self.jwt_secret)
                        .map_err(|e| Status::internal(format!("Decrypt error: {}", e)))?;

                Ok(Response::new(BotConfigWithSecretsResponse {
                    id,
                    provider,
                    name,
                    client_id,
                    client_secret,
                    service_account,
                    private_key,
                    bot_id,
                }))
            }
            None => Err(Status::not_found("Bot config not found or disabled")),
        }
    }
}
