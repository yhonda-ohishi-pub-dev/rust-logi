use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::middleware::AuthenticatedUser;
use crate::proto::common::Empty;
use crate::proto::organization::organization_service_server::OrganizationService;
use crate::proto::organization::{
    ListOrganizationsResponse, Organization, OrganizationResponse, UpdateOrganizationRequest,
};

pub struct OrganizationServiceImpl {
    pool: PgPool,
}

impl OrganizationServiceImpl {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn get_authenticated_user<T>(request: &Request<T>) -> Result<AuthenticatedUser, Status> {
        request
            .extensions()
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Authentication required"))
    }
}

#[tonic::async_trait]
impl OrganizationService for OrganizationServiceImpl {
    async fn list_my_organizations(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ListOrganizationsResponse>, Status> {
        let user = Self::get_authenticated_user(&request)?;

        let rows: Vec<(String, String, String, String, chrono::DateTime<chrono::Utc>)> =
            sqlx::query_as(
                "SELECT o.id::text, o.name, o.slug, uo.role, o.created_at
                 FROM organizations o
                 JOIN user_organizations uo ON uo.organization_id = o.id
                 WHERE uo.user_id = $1::uuid
                   AND o.deleted_at IS NULL
                 ORDER BY o.created_at",
            )
            .bind(&user.user_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let organizations = rows
            .into_iter()
            .map(|(id, name, slug, role, created_at)| Organization {
                id,
                name,
                slug,
                role,
                created_at: created_at.to_rfc3339(),
            })
            .collect();

        Ok(Response::new(ListOrganizationsResponse { organizations }))
    }

    async fn update_organization(
        &self,
        request: Request<UpdateOrganizationRequest>,
    ) -> Result<Response<OrganizationResponse>, Status> {
        let user = Self::get_authenticated_user(&request)?;
        let req = request.into_inner();

        if req.organization_id.is_empty() {
            return Err(Status::invalid_argument("organization_id is required"));
        }

        // Verify caller is admin of this organization
        let role: Option<(String,)> = sqlx::query_as(
            "SELECT role FROM user_organizations WHERE user_id = $1::uuid AND organization_id = $2::uuid",
        )
        .bind(&user.user_id)
        .bind(&req.organization_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match role {
            Some((r,)) if r == "admin" => {}
            Some(_) => return Err(Status::permission_denied("Admin role required")),
            None => {
                return Err(Status::permission_denied(
                    "Not a member of this organization",
                ))
            }
        }

        // Update
        let row: Option<(String, String, String, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "UPDATE organizations SET name = $1, slug = $2, updated_at = NOW()
             WHERE id = $3::uuid AND deleted_at IS NULL
             RETURNING id::text, name, slug, created_at",
        )
        .bind(&req.name)
        .bind(&req.slug)
        .bind(&req.organization_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                Status::already_exists("Organization slug already taken")
            } else {
                Status::internal(format!("Database error: {}", e))
            }
        })?;

        let (id, name, slug, created_at) =
            row.ok_or_else(|| Status::not_found("Organization not found"))?;

        Ok(Response::new(OrganizationResponse {
            organization: Some(Organization {
                id,
                name,
                slug,
                role: "admin".to_string(),
                created_at: created_at.to_rfc3339(),
            }),
        }))
    }
}
