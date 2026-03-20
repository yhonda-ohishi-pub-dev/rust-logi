use argon2::{Argon2, PasswordHash, PasswordVerifier};
use chrono::Utc;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tonic::{Request, Response, Status};

use crate::google_auth::GoogleTokenVerifier;
use crate::proto::auth::auth_service_server::AuthService;
use crate::middleware::AuthenticatedUser;
use crate::proto::auth::{
    AuthResponse, LoginRequest, LoginWithGoogleRequest, LoginWithSsoProviderRequest,
    ResolveSsoProviderRequest, ResolveSsoProviderResponse, SignUpWithGoogleRequest,
    SwitchOrganizationRequest, ValidateTokenRequest, ValidateTokenResponse,
};
use crate::services::lineworks_auth;
use crate::services::sso_providers;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    #[serde(default)]
    pub org: String,
    #[serde(default)]
    pub username: String,
    pub exp: i64,
    pub iat: i64,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub org_slug: String,
    // rust-alc-api JWT fields
    #[serde(default)]
    pub tenant_id: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

impl Claims {
    /// tenant_id 優先、フォールバックで org
    pub fn effective_org_id(&self) -> &str {
        self.tenant_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.org)
    }
}

pub struct AuthServiceImpl {
    pool: PgPool,
    jwt_secret: String,
    google_verifier: Option<GoogleTokenVerifier>,
    http_client: reqwest::Client,
}

impl AuthServiceImpl {
    pub fn new(pool: PgPool, jwt_secret: String, google_client_ids: Vec<String>) -> Self {
        let google_verifier = if google_client_ids.is_empty() {
            None
        } else {
            Some(GoogleTokenVerifier::new(google_client_ids))
        };
        Self {
            pool,
            jwt_secret,
            google_verifier,
            http_client: reqwest::Client::new(),
        }
    }

    fn issue_jwt(
        &self,
        user_id: &str,
        org_id: &str,
        username: &str,
        provider: &str,
        org_slug: &str,
    ) -> Result<(String, chrono::DateTime<Utc>), Status> {
        let now = Utc::now();
        let exp = now + chrono::Duration::hours(24);
        let claims = Claims {
            sub: user_id.to_string(),
            org: org_id.to_string(),
            username: username.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            provider: provider.to_string(),
            org_slug: org_slug.to_string(),
            tenant_id: None,
            email: None,
            name: None,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )
        .map_err(|e| Status::internal(format!("JWT error: {}", e)))?;
        Ok((token, exp))
    }

    fn get_google_verifier(&self) -> Result<&GoogleTokenVerifier, Status> {
        self.google_verifier.as_ref().ok_or_else(|| {
            Status::unavailable("Google authentication not configured (GOOGLE_CLIENT_ID not set)")
        })
    }
}

#[tonic::async_trait]
impl AuthService for AuthServiceImpl {
    async fn sign_up_with_google(
        &self,
        request: Request<SignUpWithGoogleRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();
        let verifier = self.get_google_verifier()?;

        // 1. Verify Google ID token
        let google_claims = verifier
            .verify(&req.id_token)
            .await
            .map_err(|e| Status::unauthenticated(format!("Google auth failed: {}", e)))?;

        // 2. Check if user already exists via oauth_accounts (SECURITY DEFINER)
        let existing: Option<(String, String, String, String)> = sqlx::query_as(
            "SELECT * FROM find_google_user($1)",
        )
        .bind(&google_claims.sub)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        if let Some((existing_user_id, org_id, email, org_slug)) = existing {
            // User already exists — treat as login
            let (token, exp) = self.issue_jwt(&existing_user_id, &org_id, &email, "google", &org_slug)?;
            return Ok(Response::new(AuthResponse {
                token,
                expires_at: exp.to_rfc3339(),
                user_id: existing_user_id,
                organization_id: org_id,
            }));
        }

        // Validate slug
        if req.organization_slug.is_empty() {
            return Err(Status::invalid_argument("Organization slug is required"));
        }
        if req.organization_name.is_empty() {
            return Err(Status::invalid_argument("Organization name is required"));
        }

        // 3. Create new user + org via SECURITY DEFINER function
        let (user_id, org_id): (String, String) = sqlx::query_as(
            "SELECT * FROM signup_create_user_and_org($1, $2, $3, 'google', $4, $5, $6)",
        )
        .bind(&google_claims.email)
        .bind(google_claims.name.as_deref().unwrap_or(&google_claims.email))
        .bind(google_claims.picture.as_deref())
        .bind(&google_claims.sub)
        .bind(&req.organization_name)
        .bind(&req.organization_slug)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            let msg = e.to_string();
            if msg.contains("unique") || msg.contains("duplicate") {
                Status::already_exists("Organization slug already taken")
            } else {
                Status::internal(format!("Failed to create user: {}", e))
            }
        })?;

        let (token, exp) = self.issue_jwt(&user_id, &org_id, &google_claims.email, "google", &req.organization_slug)?;

        Ok(Response::new(AuthResponse {
            token,
            expires_at: exp.to_rfc3339(),
            user_id,
            organization_id: org_id,
        }))
    }

    async fn login_with_google(
        &self,
        request: Request<LoginWithGoogleRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();
        let verifier = self.get_google_verifier()?;

        // 1. Verify Google ID token
        let google_claims = verifier
            .verify(&req.id_token)
            .await
            .map_err(|e| Status::unauthenticated(format!("Google auth failed: {}", e)))?;

        // 2. Look up user via SECURITY DEFINER function
        let row: Option<(String, String, String, String)> = sqlx::query_as(
            "SELECT * FROM find_google_user($1)",
        )
        .bind(&google_claims.sub)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (user_id, org_id, email, org_slug) = if let Some(row) = row {
            row
        } else {
            // Auto-register: create user in default organization via SECURITY DEFINER
            let default_org_id = "00000000-0000-0000-0000-000000000001";
            let (new_user_id, default_org_slug): (String, String) = sqlx::query_as(
                "SELECT * FROM auto_register_user($1, $2, $3, 'google', $4, NULL, $5::uuid, 'admin')",
            )
            .bind(&google_claims.email)
            .bind(google_claims.name.as_deref().unwrap_or(&google_claims.email))
            .bind(google_claims.picture.as_deref())
            .bind(&google_claims.sub)
            .bind(default_org_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Status::internal(format!("Failed to create user: {}", e)))?;

            tracing::info!("Auto-registered Google user {} in default org", &google_claims.email);
            (new_user_id, default_org_id.to_string(), google_claims.email.clone(), default_org_slug)
        };

        let (token, exp) = self.issue_jwt(&user_id, &org_id, &email, "google", &org_slug)?;

        Ok(Response::new(AuthResponse {
            token,
            expires_at: exp.to_rfc3339(),
            user_id,
            organization_id: org_id,
        }))
    }

    async fn login(
        &self,
        request: Request<LoginRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();

        if req.organization_id.is_empty() || req.username.is_empty() || req.password.is_empty() {
            return Err(Status::invalid_argument(
                "organization_id, username, and password are required",
            ));
        }

        // Query via SECURITY DEFINER function (app_users + organizations have RLS)
        let row: Option<(String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT * FROM find_password_user($1::uuid, $2)",
        )
        .bind(&req.organization_id)
        .bind(&req.username)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (app_user_id, password_hash, email, org_slug) =
            row.ok_or_else(|| Status::unauthenticated("Invalid credentials"))?;

        // Verify password with argon2
        let parsed_hash = PasswordHash::new(&password_hash)
            .map_err(|_| Status::internal("Invalid password hash in database"))?;
        Argon2::default()
            .verify_password(req.password.as_bytes(), &parsed_hash)
            .map_err(|_| Status::unauthenticated("Invalid credentials"))?;

        let username = email.as_deref().unwrap_or(&req.username);
        let (token, exp) = self.issue_jwt(&app_user_id, &req.organization_id, username, "password", &org_slug)?;

        Ok(Response::new(AuthResponse {
            token,
            expires_at: exp.to_rfc3339(),
            user_id: app_user_id,
            organization_id: req.organization_id,
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
                organization_id: data.claims.effective_org_id().to_string(),
                user_id: data.claims.sub,
                username: data.claims.username,
            })),
            Err(_) => Ok(Response::new(ValidateTokenResponse {
                valid: false,
                organization_id: String::new(),
                user_id: String::new(),
                username: String::new(),
            })),
        }
    }

    async fn resolve_sso_provider(
        &self,
        request: Request<ResolveSsoProviderRequest>,
    ) -> Result<Response<ResolveSsoProviderResponse>, Status> {
        let req = request.into_inner();

        if req.provider.is_empty() || req.external_org_id.is_empty() {
            return Err(Status::invalid_argument(
                "provider and external_org_id are required",
            ));
        }

        // Validate provider
        let provider = sso_providers::Provider::from_str(&req.provider).ok_or_else(|| {
            Status::invalid_argument(format!("Unknown provider: {}", req.provider))
        })?;

        // Use SECURITY DEFINER function to bypass RLS (pre-auth: org unknown)
        let row: Option<(String, String, Option<String>)> = sqlx::query_as(
            "SELECT * FROM resolve_sso_config($1, $2)",
        )
        .bind(&req.provider)
        .bind(&req.external_org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        match row {
            Some((client_id, org_name, woff_id)) => {
                Ok(Response::new(ResolveSsoProviderResponse {
                    available: true,
                    client_id,
                    organization_name: org_name,
                    provider: req.provider,
                    external_org_id: req.external_org_id,
                    authorize_url: provider.authorize_url().to_string(),
                    woff_id: woff_id.unwrap_or_default(),
                }))
            }
            None => Ok(Response::new(ResolveSsoProviderResponse {
                available: false,
                client_id: String::new(),
                organization_name: String::new(),
                provider: String::new(),
                external_org_id: String::new(),
                authorize_url: String::new(),
                woff_id: String::new(),
            })),
        }
    }

    async fn login_with_sso_provider(
        &self,
        request: Request<LoginWithSsoProviderRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let req = request.into_inner();

        let use_access_token = !req.access_token.is_empty();

        if req.provider.is_empty() || req.external_org_id.is_empty() {
            return Err(Status::invalid_argument(
                "provider and external_org_id are required",
            ));
        }
        if !use_access_token && (req.code.is_empty() || req.redirect_uri.is_empty()) {
            return Err(Status::invalid_argument(
                "code and redirect_uri are required (or provide access_token)",
            ));
        }

        // Validate provider
        let provider = sso_providers::Provider::from_str(&req.provider).ok_or_else(|| {
            Status::invalid_argument(format!("Unknown provider: {}", req.provider))
        })?;

        // 1. Look up SSO config — SECURITY DEFINER function to bypass RLS (pre-auth)
        let config_row: Option<(String, String, String, String)> = sqlx::query_as(
            "SELECT * FROM lookup_sso_config_for_login($1, $2)",
        )
        .bind(&req.provider)
        .bind(&req.external_org_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (client_id, client_secret_encrypted, org_id, org_slug) = config_row.ok_or_else(|| {
            Status::not_found(format!(
                "SSO config not found for provider={}, external_org_id={}",
                req.provider, req.external_org_id
            ))
        })?;

        // 2. Get access_token: either from WOFF directly or via code exchange
        let access_token = if use_access_token {
            // WOFF flow: access_token provided directly (skip code exchange)
            tracing::info!("SSO login via WOFF access_token for provider={}, external_org_id={}", req.provider, req.external_org_id);
            req.access_token.clone()
        } else {
            // Standard OAuth flow: exchange code for access_token
            let client_secret =
                lineworks_auth::decrypt_secret(&client_secret_encrypted, &self.jwt_secret)
                    .map_err(|e| Status::internal(format!("Failed to decrypt client secret: {}", e)))?;
            sso_providers::exchange_code(
                &self.http_client,
                &provider,
                &client_id,
                &client_secret,
                &req.code,
                &req.redirect_uri,
            )
            .await
            .map_err(|e| Status::unauthenticated(format!("SSO auth failed: {}", e)))?
        };

        // 4. Fetch user profile (generic)
        let profile =
            sso_providers::fetch_user_profile(&self.http_client, &provider, &access_token)
                .await
                .map_err(|e| Status::internal(format!("Failed to fetch SSO profile: {}", e)))?;

        tracing::info!(
            "SSO login: provider={}, user_id={}, email={:?}, external_org_id={}",
            req.provider,
            profile.provider_user_id,
            profile.email,
            req.external_org_id
        );

        // 5. Look up existing oauth_account via SECURITY DEFINER function
        let existing: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT * FROM find_oauth_user($1, $2)",
        )
        .bind(&req.provider)
        .bind(&profile.provider_user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (user_id, email) = if let Some((uid, email)) = existing {
            // Existing user — ensure they're still a member of this org (SECURITY DEFINER)
            sqlx::query_scalar::<_, bool>(
                "SELECT * FROM ensure_org_membership($1::uuid, $2::uuid)",
            )
            .bind(&uid)
            .bind(&org_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

            let username = email
                .clone()
                .unwrap_or_else(|| profile.display_name.clone());
            (uid, username)
        } else {
            // 6. Auto-register new user via SECURITY DEFINER function
            let user_email = profile.email.as_deref();

            let (new_user_id, _org_slug): (String, String) = sqlx::query_as(
                "SELECT * FROM auto_register_user($1, $2, NULL, $3, $4, $5, $6::uuid)",
            )
            .bind(user_email)
            .bind(&profile.display_name)
            .bind(&req.provider)
            .bind(&profile.provider_user_id)
            .bind(&access_token)
            .bind(&org_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| Status::internal(format!("Failed to create user: {}", e)))?;

            tracing::info!(
                "Auto-registered SSO user {} ({}) via provider={} in org {}",
                &profile.provider_user_id,
                &profile.display_name,
                &req.provider,
                &org_id
            );

            let username = user_email
                .map(|e| e.to_string())
                .unwrap_or_else(|| profile.display_name.clone());
            (new_user_id, username)
        };

        let (token, exp) = self.issue_jwt(&user_id, &org_id, &email, &req.provider, &org_slug)?;

        Ok(Response::new(AuthResponse {
            token,
            expires_at: exp.to_rfc3339(),
            user_id,
            organization_id: org_id,
        }))
    }

    async fn switch_organization(
        &self,
        request: Request<SwitchOrganizationRequest>,
    ) -> Result<Response<AuthResponse>, Status> {
        let auth_user = request
            .extensions()
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| Status::unauthenticated("Authentication required"))?;

        let req = request.into_inner();

        if req.organization_id.is_empty() {
            return Err(Status::invalid_argument("organization_id is required"));
        }

        // Verify membership + get user info via SECURITY DEFINER function
        let row: Option<(String, String, String)> = sqlx::query_as(
            "SELECT * FROM get_user_org_for_switch($1::uuid, $2::uuid)",
        )
        .bind(&auth_user.user_id)
        .bind(&req.organization_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Status::internal(format!("Database error: {}", e)))?;

        let (username, org_slug, _role) = row.ok_or_else(|| {
            Status::permission_denied("Not a member of the requested organization")
        })?;

        let (token, exp) = self.issue_jwt(
            &auth_user.user_id,
            &req.organization_id,
            &username,
            &auth_user.provider,
            &org_slug,
        )?;

        Ok(Response::new(AuthResponse {
            token,
            expires_at: exp.to_rfc3339(),
            user_id: auth_user.user_id,
            organization_id: req.organization_id,
        }))
    }
}
