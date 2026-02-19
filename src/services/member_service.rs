use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::middleware::AuthenticatedUser;
use crate::proto::auth::AuthResponse;
use crate::proto::common::Empty;
use crate::proto::member::member_service_server::MemberService;
use crate::proto::member::{
    AcceptInvitationRequest, InviteUserRequest, InviteUserResponse, ListMembersResponse, Member,
    MemberIdRequest, MemberResponse, RemoveMemberRequest, TransferAdminRequest,
};
use crate::services::auth_service::Claims;

pub struct MemberServiceImpl {
    pool: PgPool,
    jwt_secret: String,
}

impl MemberServiceImpl {
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

    async fn count_admins(&self, org_id: &str) -> Result<i64, Status> {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM user_organizations WHERE organization_id = $1::uuid AND role = 'admin'",
        )
        .bind(org_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(count)
    }

    fn issue_jwt(
        &self,
        user_id: &str,
        org_id: &str,
        username: &str,
    ) -> Result<(String, chrono::DateTime<Utc>), Status> {
        let now = Utc::now();
        let exp = now + chrono::Duration::hours(24);
        let claims = Claims {
            sub: user_id.to_string(),
            org: org_id.to_string(),
            username: username.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| Status::internal(format!("JWT error: {}", e)))?;
        Ok((token, exp))
    }

    async fn fetch_member(&self, user_id: &str, org_id: &str) -> Result<Member, Status> {
        let row: (String, Option<String>, String, String, chrono::DateTime<chrono::Utc>) =
            sqlx::query_as(
                "SELECT u.id::text, u.email, u.display_name, uo.role, uo.created_at
                 FROM user_organizations uo
                 JOIN app_users u ON u.id = uo.user_id
                 WHERE uo.user_id = $1::uuid AND uo.organization_id = $2::uuid",
            )
            .bind(user_id)
            .bind(org_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Member {
            user_id: row.0,
            email: row.1.unwrap_or_default(),
            display_name: row.2,
            role: row.3,
            joined_at: row.4.to_rfc3339(),
        })
    }
}

#[tonic::async_trait]
impl MemberService for MemberServiceImpl {
    async fn invite_user(
        &self,
        request: Request<InviteUserRequest>,
    ) -> Result<Response<InviteUserResponse>, Status> {
        let user = Self::get_authenticated_user(&request)?;
        let org_id = user.org_id.clone();
        let invited_by = user.user_id.clone();
        let req = request.into_inner();

        self.verify_admin(&invited_by, &org_id).await?;

        if req.email.is_empty() {
            return Err(Status::invalid_argument("Email is required"));
        }

        let role = if req.role.is_empty() {
            "member"
        } else {
            &req.role
        };
        if role != "admin" && role != "member" {
            return Err(Status::invalid_argument(
                "Role must be 'admin' or 'member'",
            ));
        }

        let token = uuid::Uuid::new_v4().to_string();
        let expires_at = Utc::now() + chrono::Duration::days(7);

        let (invitation_id,): (String,) = sqlx::query_as(
            "INSERT INTO invitations (organization_id, email, role, token, invited_by, expires_at)
             VALUES ($1::uuid, $2, $3, $4, $5::uuid, $6)
             RETURNING id::text",
        )
        .bind(&org_id)
        .bind(&req.email)
        .bind(role)
        .bind(&token)
        .bind(&invited_by)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Failed to create invitation: {}", e)))?;

        Ok(Response::new(InviteUserResponse {
            invitation_id,
            token,
            expires_at: expires_at.to_rfc3339(),
        }))
    }

    async fn accept_invitation(
        &self,
        request: Request<AcceptInvitationRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();

        if req.token.is_empty() || req.username.is_empty() || req.password.is_empty() {
            return Err(Status::invalid_argument(
                "token, username, and password are required",
            ));
        }

        // 1. Find valid invitation
        let inv: Option<(String, String, String, String)> = sqlx::query_as(
            "SELECT id::text, organization_id::text, email, role
             FROM invitations
             WHERE token = $1 AND accepted_at IS NULL AND expires_at > NOW()",
        )
        .bind(&req.token)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (inv_id, org_id, inv_email, inv_role) =
            inv.ok_or_else(|| Status::not_found("Invalid or expired invitation"))?;

        // 2. Hash password
        let salt = SaltString::generate(&mut OsRng);
        let password_hash = Argon2::default()
            .hash_password(req.password.as_bytes(), &salt)
            .map_err(|e| Status::internal(format!("Password hash error: {}", e)))?
            .to_string();

        // 3. Transaction: create user + credentials + membership
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| Status::internal(format!("Transaction error: {}", e)))?;

        let display_name = if req.display_name.is_empty() {
            inv_email.clone()
        } else {
            req.display_name.clone()
        };

        // Check if app_user already exists for this email
        let existing_user: Option<(String,)> = sqlx::query_as(
            "SELECT id::text FROM app_users WHERE email = $1 AND deleted_at IS NULL",
        )
        .bind(&inv_email)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let user_id = if let Some((id,)) = existing_user {
            id
        } else {
            // Create new app_user
            let (id,): (String,) = sqlx::query_as(
                "INSERT INTO app_users (email, display_name) VALUES ($1, $2) RETURNING id::text",
            )
            .bind(&inv_email)
            .bind(&display_name)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| Status::internal(format!("Failed to create user: {}", e)))?;
            id
        };

        // Create password_credentials
        sqlx::query(
            "INSERT INTO password_credentials (app_user_id, organization_id, username, password_hash)
             VALUES ($1::uuid, $2::uuid, $3, $4)",
        )
        .bind(&user_id)
        .bind(&org_id)
        .bind(&req.username)
        .bind(&password_hash)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                Status::already_exists("Username already taken in this organization")
            } else {
                Status::internal(format!("Failed to create credentials: {}", e))
            }
        })?;

        // Create user_organizations (or update if already exists)
        sqlx::query(
            "INSERT INTO user_organizations (user_id, organization_id, role, is_default)
             VALUES ($1::uuid, $2::uuid, $3, true)
             ON CONFLICT (user_id, organization_id)
             DO UPDATE SET role = EXCLUDED.role, updated_at = NOW()",
        )
        .bind(&user_id)
        .bind(&org_id)
        .bind(&inv_role)
        .execute(&mut *tx)
        .await
        .map_err(|e| Status::internal(format!("Failed to create membership: {}", e)))?;

        // Mark invitation as accepted
        sqlx::query(
            "UPDATE invitations SET accepted_by = $1::uuid, accepted_at = NOW() WHERE id = $2::uuid",
        )
        .bind(&user_id)
        .bind(&inv_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| Status::internal(format!("Failed to update invitation: {}", e)))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(format!("Transaction commit error: {}", e)))?;

        // Issue JWT (auto-login)
        let (token, exp) = self.issue_jwt(&user_id, &org_id, &inv_email)?;

        Ok(Response::new(AuthResponse {
            token,
            expires_at: exp.to_rfc3339(),
            user_id,
            organization_id: org_id,
        }))
    }

    async fn list_members(
        &self,
        request: Request<Empty>,
    ) -> Result<Response<ListMembersResponse>, Status> {
        let user = Self::get_authenticated_user(&request)?;
        let org_id = user.org_id.clone();

        let rows: Vec<(
            String,
            Option<String>,
            String,
            String,
            chrono::DateTime<chrono::Utc>,
        )> = sqlx::query_as(
            "SELECT u.id::text, u.email, u.display_name, uo.role, uo.created_at
             FROM user_organizations uo
             JOIN app_users u ON u.id = uo.user_id
             WHERE uo.organization_id = $1::uuid
               AND u.deleted_at IS NULL
             ORDER BY uo.created_at",
        )
        .bind(&org_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let members = rows
            .into_iter()
            .map(|(user_id, email, display_name, role, joined_at)| Member {
                user_id,
                email: email.unwrap_or_default(),
                display_name,
                role,
                joined_at: joined_at.to_rfc3339(),
            })
            .collect();

        Ok(Response::new(ListMembersResponse { members }))
    }

    async fn remove_member(
        &self,
        request: Request<RemoveMemberRequest>,
    ) -> Result<Response<Empty>, Status> {
        let user = Self::get_authenticated_user(&request)?;
        let org_id = user.org_id.clone();
        let caller_id = user.user_id.clone();
        let req = request.into_inner();

        self.verify_admin(&caller_id, &org_id).await?;

        if req.user_id == caller_id {
            return Err(Status::invalid_argument(
                "Cannot remove yourself. Use TransferAdmin or DemoteFromAdmin first.",
            ));
        }

        // Check if target is admin and is the last admin
        let target_role: Option<(String,)> = sqlx::query_as(
            "SELECT role FROM user_organizations WHERE user_id = $1::uuid AND organization_id = $2::uuid",
        )
        .bind(&req.user_id)
        .bind(&org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (target_role_str,) = target_role
            .ok_or_else(|| Status::not_found("User is not a member of this organization"))?;

        if target_role_str == "admin" {
            let admin_count = self.count_admins(&org_id).await?;
            if admin_count <= 1 {
                return Err(Status::failed_precondition(
                    "Cannot remove the last admin. Transfer admin role first.",
                ));
            }
        }

        // Remove from user_organizations
        sqlx::query(
            "DELETE FROM user_organizations WHERE user_id = $1::uuid AND organization_id = $2::uuid",
        )
        .bind(&req.user_id)
        .bind(&org_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        // Remove password_credentials for this org
        sqlx::query(
            "DELETE FROM password_credentials WHERE app_user_id = $1::uuid AND organization_id = $2::uuid",
        )
        .bind(&req.user_id)
        .bind(&org_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(Empty {}))
    }

    async fn promote_to_admin(
        &self,
        request: Request<MemberIdRequest>,
    ) -> Result<Response<MemberResponse>, Status> {
        let user = Self::get_authenticated_user(&request)?;
        let org_id = user.org_id.clone();
        let caller_id = user.user_id.clone();
        let req = request.into_inner();

        self.verify_admin(&caller_id, &org_id).await?;

        let result = sqlx::query(
            "UPDATE user_organizations SET role = 'admin', updated_at = NOW()
             WHERE user_id = $1::uuid AND organization_id = $2::uuid",
        )
        .bind(&req.user_id)
        .bind(&org_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(Status::not_found(
                "User is not a member of this organization",
            ));
        }

        let member = self.fetch_member(&req.user_id, &org_id).await?;
        Ok(Response::new(MemberResponse {
            member: Some(member),
        }))
    }

    async fn demote_from_admin(
        &self,
        request: Request<MemberIdRequest>,
    ) -> Result<Response<MemberResponse>, Status> {
        let user = Self::get_authenticated_user(&request)?;
        let org_id = user.org_id.clone();
        let caller_id = user.user_id.clone();
        let req = request.into_inner();

        self.verify_admin(&caller_id, &org_id).await?;

        // Check admin count before demotion
        let admin_count = self.count_admins(&org_id).await?;
        if admin_count <= 1 {
            return Err(Status::failed_precondition(
                "Cannot demote the last admin. Use TransferAdmin instead.",
            ));
        }

        let result = sqlx::query(
            "UPDATE user_organizations SET role = 'member', updated_at = NOW()
             WHERE user_id = $1::uuid AND organization_id = $2::uuid AND role = 'admin'",
        )
        .bind(&req.user_id)
        .bind(&org_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(Status::not_found(
                "User is not an admin of this organization",
            ));
        }

        let member = self.fetch_member(&req.user_id, &org_id).await?;
        Ok(Response::new(MemberResponse {
            member: Some(member),
        }))
    }

    async fn transfer_admin(
        &self,
        request: Request<TransferAdminRequest>,
    ) -> Result<Response<Empty>, Status> {
        let user = Self::get_authenticated_user(&request)?;
        let org_id = user.org_id.clone();
        let caller_id = user.user_id.clone();
        let req = request.into_inner();

        self.verify_admin(&caller_id, &org_id).await?;

        if req.target_user_id == caller_id {
            return Err(Status::invalid_argument(
                "Cannot transfer admin to yourself",
            ));
        }

        // Verify target is a member
        let target_exists: Option<(String,)> = sqlx::query_as(
            "SELECT role FROM user_organizations WHERE user_id = $1::uuid AND organization_id = $2::uuid",
        )
        .bind(&req.target_user_id)
        .bind(&org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (target_role,) = target_exists.ok_or_else(|| {
            Status::not_found("Target user is not a member of this organization")
        })?;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| Status::internal(format!("Transaction error: {}", e)))?;

        // Promote target to admin (if not already)
        if target_role != "admin" {
            sqlx::query(
                "UPDATE user_organizations SET role = 'admin', updated_at = NOW()
                 WHERE user_id = $1::uuid AND organization_id = $2::uuid",
            )
            .bind(&req.target_user_id)
            .bind(&org_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;
        }

        // Demote caller to member
        sqlx::query(
            "UPDATE user_organizations SET role = 'member', updated_at = NOW()
             WHERE user_id = $1::uuid AND organization_id = $2::uuid",
        )
        .bind(&caller_id)
        .bind(&org_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        tx.commit()
            .await
            .map_err(|e| Status::internal(format!("Transaction commit error: {}", e)))?;

        Ok(Response::new(Empty {}))
    }
}
