use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::db::organization::set_current_organization;
use crate::middleware::AuthenticatedUser;
use crate::proto::sso_settings::sso_settings_service_server::SsoSettingsService;
use crate::proto::sso_settings::{
    DeleteSsoConfigRequest, DeleteSsoConfigResponse, GetSsoConfigRequest, ListSsoConfigsRequest,
    ListSsoConfigsResponse, SsoConfigResponse, UpsertSsoConfigRequest,
};
use crate::services::lineworks_auth;

pub struct SsoSettingsServiceImpl {
    pool: PgPool,
    jwt_secret: String,
}

impl SsoSettingsServiceImpl {
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
            None => Err(Status::permission_denied(
                "Not a member of this organization",
            )),
        }
    }
}

#[tonic::async_trait]
impl SsoSettingsService for SsoSettingsServiceImpl {
    async fn get_config(
        &self,
        request: Request<GetSsoConfigRequest>,
    ) -> Result<Response<SsoConfigResponse>, Status> {
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

        let row: Option<(String, String, String, bool, String, String, Option<String>)> = sqlx::query_as(
            "SELECT provider, client_id, external_org_id, enabled,
                    created_at::text, updated_at::text, woff_id
             FROM sso_provider_configs
             WHERE organization_id = $1::uuid AND provider = $2",
        )
        .bind(&auth_user.org_id)
        .bind(&req.provider)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match row {
            Some((provider, client_id, external_org_id, enabled, created_at, updated_at, woff_id)) => {
                Ok(Response::new(SsoConfigResponse {
                    provider,
                    client_id,
                    has_client_secret: true,
                    external_org_id,
                    enabled,
                    created_at,
                    updated_at,
                    woff_id: woff_id.unwrap_or_default(),
                }))
            }
            None => Ok(Response::new(SsoConfigResponse {
                provider: req.provider,
                client_id: String::new(),
                has_client_secret: false,
                external_org_id: String::new(),
                enabled: false,
                created_at: String::new(),
                updated_at: String::new(),
                woff_id: String::new(),
            })),
        }
    }

    async fn upsert_config(
        &self,
        request: Request<UpsertSsoConfigRequest>,
    ) -> Result<Response<SsoConfigResponse>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        self.verify_admin(&auth_user.user_id, &auth_user.org_id)
            .await?;

        let req = request.into_inner();

        if req.client_id.is_empty() || req.external_org_id.is_empty() || req.provider.is_empty() {
            return Err(Status::invalid_argument(
                "provider, client_id, and external_org_id are required",
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

        // Check if config already exists for this provider
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id::text FROM sso_provider_configs WHERE organization_id = $1::uuid AND provider = $2",
        )
        .bind(&auth_user.org_id)
        .bind(&req.provider)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let woff_id_val = if req.woff_id.is_empty() { None } else { Some(&req.woff_id) };

        if let Some(_) = existing {
            // Update existing config
            if req.client_secret.is_empty() {
                // Update without changing secret
                sqlx::query(
                    "UPDATE sso_provider_configs
                     SET client_id = $1, external_org_id = $2, enabled = $3, woff_id = $4, updated_at = NOW()
                     WHERE organization_id = $5::uuid AND provider = $6",
                )
                .bind(&req.client_id)
                .bind(&req.external_org_id)
                .bind(req.enabled)
                .bind(woff_id_val)
                .bind(&auth_user.org_id)
                .bind(&req.provider)
                .execute(&mut *conn)
                .await
                .map_err(|e| Status::internal(format!("Update error: {}", e)))?;
            } else {
                // Update with new secret
                let encrypted =
                    lineworks_auth::encrypt_secret(&req.client_secret, &self.jwt_secret)
                        .map_err(|e| {
                            Status::internal(format!("Failed to encrypt secret: {}", e))
                        })?;
                sqlx::query(
                    "UPDATE sso_provider_configs
                     SET client_id = $1, client_secret_encrypted = $2, external_org_id = $3,
                         enabled = $4, woff_id = $5, updated_at = NOW()
                     WHERE organization_id = $6::uuid AND provider = $7",
                )
                .bind(&req.client_id)
                .bind(&encrypted)
                .bind(&req.external_org_id)
                .bind(req.enabled)
                .bind(woff_id_val)
                .bind(&auth_user.org_id)
                .bind(&req.provider)
                .execute(&mut *conn)
                .await
                .map_err(|e| Status::internal(format!("Update error: {}", e)))?;
            }
        } else {
            // Create new config (client_secret required)
            if req.client_secret.is_empty() {
                return Err(Status::invalid_argument(
                    "client_secret is required for initial setup",
                ));
            }

            let encrypted =
                lineworks_auth::encrypt_secret(&req.client_secret, &self.jwt_secret).map_err(
                    |e| Status::internal(format!("Failed to encrypt secret: {}", e)),
                )?;

            sqlx::query(
                "INSERT INTO sso_provider_configs
                 (organization_id, provider, client_id, client_secret_encrypted, external_org_id, enabled, woff_id)
                 VALUES ($1::uuid, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&auth_user.org_id)
            .bind(&req.provider)
            .bind(&req.client_id)
            .bind(&encrypted)
            .bind(&req.external_org_id)
            .bind(req.enabled)
            .bind(woff_id_val)
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                    Status::already_exists(
                        "This external_org_id or client_id is already configured for another organization",
                    )
                } else {
                    Status::internal(format!("Insert error: {}", e))
                }
            })?;
        }

        // Return updated config
        let (provider, client_id, external_org_id, enabled, created_at, updated_at, woff_id): (
            String,
            String,
            String,
            bool,
            String,
            String,
            Option<String>,
        ) = sqlx::query_as(
            "SELECT provider, client_id, external_org_id, enabled,
                    created_at::text, updated_at::text, woff_id
             FROM sso_provider_configs
             WHERE organization_id = $1::uuid AND provider = $2",
        )
        .bind(&auth_user.org_id)
        .bind(&req.provider)
        .fetch_one(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(SsoConfigResponse {
            provider,
            client_id,
            has_client_secret: true,
            external_org_id,
            enabled,
            created_at,
            updated_at,
            woff_id: woff_id.unwrap_or_default(),
        }))
    }

    async fn delete_config(
        &self,
        request: Request<DeleteSsoConfigRequest>,
    ) -> Result<Response<DeleteSsoConfigResponse>, Status> {
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

        sqlx::query(
            "DELETE FROM sso_provider_configs WHERE organization_id = $1::uuid AND provider = $2",
        )
        .bind(&auth_user.org_id)
        .bind(&req.provider)
        .execute(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Delete error: {}", e)))?;

        Ok(Response::new(DeleteSsoConfigResponse {}))
    }

    async fn list_configs(
        &self,
        request: Request<ListSsoConfigsRequest>,
    ) -> Result<Response<ListSsoConfigsResponse>, Status> {
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

        let rows: Vec<(String, String, String, bool, String, String, Option<String>)> = sqlx::query_as(
            "SELECT provider, client_id, external_org_id, enabled,
                    created_at::text, updated_at::text, woff_id
             FROM sso_provider_configs
             WHERE organization_id = $1::uuid
             ORDER BY provider",
        )
        .bind(&auth_user.org_id)
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let configs = rows
            .into_iter()
            .map(
                |(provider, client_id, external_org_id, enabled, created_at, updated_at, woff_id)| {
                    SsoConfigResponse {
                        provider,
                        client_id,
                        has_client_secret: true,
                        external_org_id,
                        enabled,
                        created_at,
                        updated_at,
                        woff_id: woff_id.unwrap_or_default(),
                    }
                },
            )
            .collect();

        Ok(Response::new(ListSsoConfigsResponse { configs }))
    }
}
