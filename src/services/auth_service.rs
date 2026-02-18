use argon2::{Argon2, PasswordHash, PasswordVerifier};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::models::ApiUser;
use crate::proto::auth::auth_service_server::AuthService;
use crate::proto::auth::{
    LoginRequest, LoginResponse, ValidateTokenRequest, ValidateTokenResponse,
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
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<LoginResponse>, Status> {
        let req = request.into_inner();

        let user = sqlx::query_as::<_, ApiUser>(
            "SELECT id::text, organization_id::text, username, password_hash, enabled
             FROM api_users WHERE username = $1",
        )
        .bind(&req.username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?
        .ok_or_else(|| Status::unauthenticated("Invalid username or password"))?;

        if !user.enabled {
            return Err(Status::unauthenticated("Account disabled"));
        }

        let parsed_hash = PasswordHash::new(&user.password_hash)
            .map_err(|e| Status::internal(format!("Invalid password hash: {}", e)))?;
        Argon2::default()
            .verify_password(req.password.as_bytes(), &parsed_hash)
            .map_err(|_| Status::unauthenticated("Invalid username or password"))?;

        let now = Utc::now();
        let exp = now + chrono::Duration::hours(24);
        let claims = Claims {
            sub: user.id.clone(),
            org: user.organization_id.clone(),
            username: user.username.clone(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| Status::internal(format!("JWT error: {}", e)))?;

        let token_hash = format!("{:x}", Sha256::digest(token.as_bytes()));
        sqlx::query(
            "INSERT INTO api_tokens (user_id, token_hash, expires_at)
             VALUES ($1::uuid, $2, $3)",
        )
        .bind(&user.id)
        .bind(&token_hash)
        .bind(exp)
        .execute(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

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

        let claims = jsonwebtoken::decode::<Claims>(
            &req.token,
            &jsonwebtoken::DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &jsonwebtoken::Validation::default(),
        )
        .map_err(|_| Status::unauthenticated("Invalid token"))?
        .claims;

        let token_hash = format!("{:x}", Sha256::digest(req.token.as_bytes()));
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM api_tokens
             WHERE token_hash = $1 AND revoked = false AND expires_at > NOW())",
        )
        .bind(&token_hash)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        Ok(Response::new(ValidateTokenResponse {
            valid: exists,
            organization_id: if exists {
                claims.org
            } else {
                String::new()
            },
            username: if exists {
                claims.username
            } else {
                String::new()
            },
        }))
    }
}
