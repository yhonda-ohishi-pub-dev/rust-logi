use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use http::header::HeaderValue;
use http::Request as HttpRequest;
use http::Response as HttpResponse;
use http_body_util::combinators::UnsyncBoxBody;
use jsonwebtoken::{DecodingKey, Validation};
use sqlx::PgPool;
use tonic::Status;
use tower::{Layer, Service};

use crate::services::auth_service::Claims;

/// Authenticated user info injected by the auth middleware into request extensions.
#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub user_id: String,
    pub org_id: String,
    pub role: String,
}

/// Public paths that do not require JWT authentication
const PUBLIC_PATHS: &[&str] = &[
    "/logi.auth.AuthService/Login",
    "/logi.auth.AuthService/SignUpWithGoogle",
    "/logi.auth.AuthService/LoginWithGoogle",
    "/logi.auth.AuthService/ValidateToken",
    "/logi.member.MemberService/AcceptInvitation",
    "/grpc.health.v1.Health/Check",
    "/grpc.health.v1.Health/Watch",
    "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo",
    "/grpc.reflection.v1alpha.ServerReflection/ServerReflectionInfo",
];

/// x-organization-id metadata key
const ORG_HEADER: &str = "x-organization-id";

#[derive(Clone)]
pub struct AuthLayer {
    pool: PgPool,
    jwt_secret: String,
}

impl AuthLayer {
    pub fn new(pool: PgPool, jwt_secret: String) -> Self {
        Self { pool, jwt_secret }
    }
}

impl<S> Layer<S> for AuthLayer {
    type Service = AuthMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AuthMiddleware {
            inner,
            pool: self.pool.clone(),
            jwt_secret: self.jwt_secret.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AuthMiddleware<S> {
    inner: S,
    pool: PgPool,
    jwt_secret: String,
}

type BoxBody = UnsyncBoxBody<bytes::Bytes, Status>;

fn grpc_status_response(status: Status) -> HttpResponse<BoxBody> {
    let code = status.code() as i32;
    let message = status.message().to_string();

    let mut response = HttpResponse::new(UnsyncBoxBody::default());
    response.headers_mut().insert(
        "content-type",
        HeaderValue::from_static("application/grpc"),
    );
    response.headers_mut().insert(
        "grpc-status",
        HeaderValue::from_str(&code.to_string()).unwrap(),
    );
    if !message.is_empty() {
        if let Ok(val) = HeaderValue::from_str(&message) {
            response.headers_mut().insert("grpc-message", val);
        }
    }
    response
}

impl<S, ReqBody> Service<HttpRequest<ReqBody>> for AuthMiddleware<S>
where
    S: Service<HttpRequest<ReqBody>, Response = HttpResponse<BoxBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = HttpResponse<BoxBody>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: HttpRequest<ReqBody>) -> Self::Future {
        let mut inner = self.inner.clone();
        std::mem::swap(&mut self.inner, &mut inner);

        let pool = self.pool.clone();
        let jwt_secret = self.jwt_secret.clone();

        Box::pin(async move {
            let path = req.uri().path().to_string();

            // Check if this is a public path
            if PUBLIC_PATHS.iter().any(|p| path == *p) {
                return inner.call(req).await;
            }

            // Extract Authorization header
            let auth_header = req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(|s| s.to_string());

            // Try JWT authentication
            let jwt_claims = auth_header.and_then(|token| {
                jsonwebtoken::decode::<Claims>(
                    &token,
                    &DecodingKey::from_secret(jwt_secret.as_bytes()),
                    &Validation::default(),
                )
                .ok()
                .map(|data| data.claims)
            });

            if let Some(claims) = jwt_claims {
                // JWT is valid — determine effective org_id
                let requested_org = req
                    .headers()
                    .get(ORG_HEADER)
                    .and_then(|v| v.to_str().ok())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

                let (effective_org_id, role) = if let Some(ref org_id) = requested_org {
                    if *org_id != claims.org {
                        // User is requesting a different org — verify membership
                        match verify_membership(&pool, &claims.sub, org_id).await {
                            Ok(role) => (org_id.clone(), role),
                            Err(_) => {
                                tracing::warn!(
                                    "User {} not a member of org {}",
                                    claims.sub,
                                    org_id
                                );
                                return Ok(grpc_status_response(Status::permission_denied(
                                    "Not a member of the requested organization",
                                )));
                            }
                        }
                    } else {
                        match verify_membership(&pool, &claims.sub, org_id).await {
                            Ok(role) => (org_id.clone(), role),
                            Err(_) => (claims.org.clone(), "member".to_string()),
                        }
                    }
                } else {
                    match verify_membership(&pool, &claims.sub, &claims.org).await {
                        Ok(role) => (claims.org.clone(), role),
                        Err(_) => (claims.org.clone(), "member".to_string()),
                    }
                };

                // Inject AuthenticatedUser into extensions
                req.extensions_mut().insert(AuthenticatedUser {
                    user_id: claims.sub,
                    org_id: effective_org_id.clone(),
                    role,
                });

                // Also set x-organization-id header so existing services can read it
                if let Ok(value) = effective_org_id.parse() {
                    req.headers_mut().insert(ORG_HEADER, value);
                }
            }
            // No valid JWT — pass through (backwards compatible)
            // Existing services use get_organization_from_request() which falls back to
            // x-organization-id header or DEFAULT_ORGANIZATION_ID

            inner.call(req).await
        })
    }
}

async fn verify_membership(pool: &PgPool, user_id: &str, org_id: &str) -> Result<String, ()> {
    sqlx::query_scalar::<_, String>(
        "SELECT role FROM user_organizations WHERE user_id = $1::uuid AND organization_id = $2::uuid",
    )
    .bind(user_id)
    .bind(org_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| ())?
    .ok_or(())
}
