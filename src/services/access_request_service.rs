use std::sync::Arc;

use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::config::Config;
use crate::db::organization::set_current_organization;
use crate::http_client::HttpClient;
use crate::middleware::AuthenticatedUser;
use crate::proto::access_request::access_request_service_server::AccessRequestService;
use crate::proto::access_request::{
    AccessRequest, ApproveAccessRequestReq, CreateAccessRequestReq, CreateAccessRequestRes,
    DeclineAccessRequestReq, GetOrgBySlugReq, GetOrgBySlugRes, ListAccessRequestsReq,
    ListAccessRequestsRes,
};
use crate::proto::common::Empty;

pub struct AccessRequestServiceImpl {
    pool: PgPool,
    config: Config,
    http_client: Arc<HttpClient>,
}

impl AccessRequestServiceImpl {
    pub fn new(pool: PgPool, config: Config, http_client: Arc<HttpClient>) -> Self {
        Self {
            pool,
            config,
            http_client,
        }
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

    async fn send_line_notification(
        &self,
        org_name: &str,
        display_name: &str,
        email: &str,
        provider: &str,
    ) {
        let bot_url = match &self.config.dvr_lineworks_bot_url {
            Some(url) => url.clone(),
            None => return,
        };

        let message = format!(
            "【参加リクエスト】\n組織: {}\nユーザー: {} ({})\nプロバイダー: {}",
            org_name, display_name, email, provider
        );

        let payload = serde_json::json!({
            "test": "sendTextMessageLine",
            "message": message
        });

        let api_url = format!("{}/api/tasks", bot_url.trim_end_matches('/'));
        let http_client = self.http_client.clone();

        tokio::spawn(async move {
            match http_client.post_json(&api_url, &payload).await {
                Ok(response) => {
                    if response.status().is_success() {
                        tracing::info!("LINE notification sent for access request");
                    } else {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        tracing::error!("LINE notification failed: {} - {}", status, body);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to send LINE notification: {}", e);
                }
            }
        });
    }
}

#[tonic::async_trait]
impl AccessRequestService for AccessRequestServiceImpl {
    async fn get_organization_by_slug(
        &self,
        request: Request<GetOrgBySlugReq>,
    ) -> Result<Response<GetOrgBySlugRes>, Status> {
        let req = request.into_inner();

        if req.slug.is_empty() {
            return Err(Status::invalid_argument("slug is required"));
        }

        let row: Option<(String, String, String)> = sqlx::query_as(
            "SELECT id::text, name, slug FROM organizations WHERE slug = $1 AND deleted_at IS NULL",
        )
        .bind(&req.slug)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match row {
            Some((id, name, slug)) => Ok(Response::new(GetOrgBySlugRes {
                found: true,
                organization_id: id,
                name,
                slug,
            })),
            None => Ok(Response::new(GetOrgBySlugRes {
                found: false,
                organization_id: String::new(),
                name: String::new(),
                slug: String::new(),
            })),
        }
    }

    async fn create_access_request(
        &self,
        request: Request<CreateAccessRequestReq>,
    ) -> Result<Response<CreateAccessRequestRes>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        let req = request.into_inner();

        if req.org_slug.is_empty() {
            return Err(Status::invalid_argument("org_slug is required"));
        }

        // Resolve organization by slug
        let org: Option<(String, String)> = sqlx::query_as(
            "SELECT id::text, name FROM organizations WHERE slug = $1 AND deleted_at IS NULL",
        )
        .bind(&req.org_slug)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (org_id, org_name) = match org {
            Some(o) => o,
            None => return Err(Status::not_found("Organization not found")),
        };

        // Check if already a member
        let is_member: Option<(i32,)> = sqlx::query_as(
            "SELECT 1 FROM user_organizations WHERE user_id = $1::uuid AND organization_id = $2::uuid",
        )
        .bind(&auth_user.user_id)
        .bind(&org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        if is_member.is_some() {
            return Ok(Response::new(CreateAccessRequestRes {
                id: String::new(),
                status: "already_member".to_string(),
                org_name,
            }));
        }

        // Check if already pending
        let pending: Option<(String,)> = sqlx::query_as(
            "SELECT id::text FROM access_requests WHERE user_id = $1::uuid AND organization_id = $2::uuid AND status = 'pending'",
        )
        .bind(&auth_user.user_id)
        .bind(&org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        if let Some((existing_id,)) = pending {
            return Ok(Response::new(CreateAccessRequestRes {
                id: existing_id,
                status: "already_pending".to_string(),
                org_name,
            }));
        }

        // Get user info
        let user_info: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT email, COALESCE(display_name, ''), avatar_url FROM app_users WHERE id = $1::uuid",
        )
        .bind(&auth_user.user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (email, display_name, avatar_url) = match user_info {
            Some(info) => info,
            None => return Err(Status::internal("User not found")),
        };

        let provider = &auth_user.provider;

        // INSERT (RLS is ENABLE not FORCE, INSERT policy is WITH CHECK(true))
        let row: (String,) = sqlx::query_as(
            "INSERT INTO access_requests (organization_id, user_id, email, display_name, avatar_url, provider) \
             VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6) RETURNING id::text",
        )
        .bind(&org_id)
        .bind(&auth_user.user_id)
        .bind(&email)
        .bind(&display_name)
        .bind(&avatar_url)
        .bind(provider)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        // Send LINE notification asynchronously
        self.send_line_notification(&org_name, &display_name, &email, provider)
            .await;

        Ok(Response::new(CreateAccessRequestRes {
            id: row.0,
            status: "pending".to_string(),
            org_name,
        }))
    }

    async fn list_access_requests(
        &self,
        request: Request<ListAccessRequestsReq>,
    ) -> Result<Response<ListAccessRequestsRes>, Status> {
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

        let rows: Vec<(String, String, String, String, Option<String>, String, String, Option<String>, Option<String>, Option<String>, String)> =
            if req.status_filter.is_empty() {
                sqlx::query_as(
                    "SELECT id::text, user_id::text, email, display_name, avatar_url, \
                     provider, status, role, reviewed_by::text, reviewed_at::text, created_at::text \
                     FROM access_requests ORDER BY created_at DESC",
                )
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            } else {
                sqlx::query_as(
                    "SELECT id::text, user_id::text, email, display_name, avatar_url, \
                     provider, status, role, reviewed_by::text, reviewed_at::text, created_at::text \
                     FROM access_requests WHERE status = $1 ORDER BY created_at DESC",
                )
                .bind(&req.status_filter)
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| Status::internal(format!("Database error: {}", e)))?
            };

        let requests: Vec<AccessRequest> = rows
            .into_iter()
            .map(
                |(id, user_id, email, display_name, avatar_url, provider, status, role, reviewed_by, reviewed_at, created_at)| {
                    AccessRequest {
                        id,
                        user_id,
                        email,
                        display_name,
                        avatar_url: avatar_url.unwrap_or_default(),
                        provider,
                        status,
                        role: role.unwrap_or_default(),
                        reviewed_by: reviewed_by.unwrap_or_default(),
                        reviewed_at: reviewed_at.unwrap_or_default(),
                        created_at,
                    }
                },
            )
            .collect();

        Ok(Response::new(ListAccessRequestsRes { requests }))
    }

    async fn approve_access_request(
        &self,
        request: Request<ApproveAccessRequestReq>,
    ) -> Result<Response<Empty>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        self.verify_admin(&auth_user.user_id, &auth_user.org_id)
            .await?;
        let req = request.into_inner();

        if req.request_id.is_empty() {
            return Err(Status::invalid_argument("request_id is required"));
        }

        let role = if req.role.is_empty() {
            "member"
        } else {
            &req.role
        };

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Pool error: {}", e)))?;
        set_current_organization(&mut conn, &auth_user.org_id)
            .await
            .map_err(|e| Status::internal(format!("RLS error: {}", e)))?;

        // Fetch the pending request
        let access_req: Option<(String, String)> = sqlx::query_as(
            "SELECT user_id::text, organization_id::text FROM access_requests \
             WHERE id = $1::uuid AND status = 'pending'",
        )
        .bind(&req.request_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (target_user_id, target_org_id) = match access_req {
            Some(r) => r,
            None => return Err(Status::not_found("Pending access request not found")),
        };

        // Update access request status
        sqlx::query(
            "UPDATE access_requests SET status = 'approved', role = $1, \
             reviewed_by = $2::uuid, reviewed_at = NOW(), updated_at = NOW() \
             WHERE id = $3::uuid",
        )
        .bind(role)
        .bind(&auth_user.user_id)
        .bind(&req.request_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        // Add user to organization (ON CONFLICT in case of race)
        sqlx::query(
            "INSERT INTO user_organizations (user_id, organization_id, role) \
             VALUES ($1::uuid, $2::uuid, $3) \
             ON CONFLICT (user_id, organization_id) DO UPDATE SET role = $3, updated_at = NOW()",
        )
        .bind(&target_user_id)
        .bind(&target_org_id)
        .bind(role)
        .execute(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(Empty {}))
    }

    async fn decline_access_request(
        &self,
        request: Request<DeclineAccessRequestReq>,
    ) -> Result<Response<Empty>, Status> {
        let auth_user = Self::get_authenticated_user(&request)?;
        self.verify_admin(&auth_user.user_id, &auth_user.org_id)
            .await?;
        let req = request.into_inner();

        if req.request_id.is_empty() {
            return Err(Status::invalid_argument("request_id is required"));
        }

        let mut conn = self
            .pool
            .acquire()
            .await
            .map_err(|e| Status::internal(format!("Pool error: {}", e)))?;
        set_current_organization(&mut conn, &auth_user.org_id)
            .await
            .map_err(|e| Status::internal(format!("RLS error: {}", e)))?;

        let rows_affected = sqlx::query(
            "UPDATE access_requests SET status = 'declined', \
             reviewed_by = $1::uuid, reviewed_at = NOW(), updated_at = NOW() \
             WHERE id = $2::uuid AND status = 'pending'",
        )
        .bind(&auth_user.user_id)
        .bind(&req.request_id)
        .execute(&mut *conn)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?
        .rows_affected();

        if rows_affected == 0 {
            return Err(Status::not_found("Pending access request not found"));
        }

        Ok(Response::new(Empty {}))
    }
}
