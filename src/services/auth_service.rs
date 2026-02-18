use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::proto::auth::auth_service_server::AuthService;
use crate::proto::auth::{
    LoginRequest, LoginResponse, LoginWithGoogleRequest, ValidateTokenRequest,
    ValidateTokenResponse,
};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    org: String,
    username: String,
    exp: i64,
    iat: i64,
}

pub struct AuthServiceImpl {
    pool: PgPool,
    jwt_secret: String,
}

impl AuthServiceImpl {
    pub fn new(pool: PgPool, jwt_secret: String) -> Self {
        Self { pool, jwt_secret }
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
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn login(
        &self,
        _request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        Err(Status::unimplemented("Use LoginWithGoogle instead"))
    }

    async fn login_with_google(
        &self,
        request: Request<LoginWithGoogleRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let req = request.into_inner();

        let row = sqlx::query!(
            r#"
            SELECT
                u.id::text AS "id!",
                uo.organization_id::text AS "organization_id!",
                u.email AS "email!"
            FROM app_users u
            JOIN user_organizations uo ON uo.user_id = u.id AND uo.is_default = true
            WHERE u.email = $1
              AND u.deleted_at IS NULL
            LIMIT 1
            "#,
            req.email,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?
        .ok_or_else(|| Status::unauthenticated("User not found"))?;

        let (token, exp) = self.issue_jwt(&row.id, &row.organization_id, &row.email)?;

        Ok(Response::new(LoginResponse {
            token,
            expires_at: exp.to_rfc3339(),
        }))
    }

    async fn validate_token(
        &self,
        request: Request<ValidateTokenRequest>,
    ) -> Result<Response<ValidateTokenResponse>, Status> {
        let req = request.into_inner();

        let result = jsonwebtoken::decode::<Claims>(
            &req.token,
            &jsonwebtoken::DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &jsonwebtoken::Validation::default(),
        );

        match result {
            Ok(data) => Ok(Response::new(ValidateTokenResponse {
                valid: true,
                organization_id: data.claims.org,
                username: data.claims.username,
            })),
            Err(_) => Ok(Response::new(ValidateTokenResponse {
                valid: false,
                organization_id: String::new(),
                username: String::new(),
            })),
        }
    }
}
