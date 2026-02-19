use sqlx::{PgConnection, PgPool, Executor};
use std::future::Future;
use tonic::metadata::MetadataMap;

use crate::middleware::AuthenticatedUser;

/// Default organization ID (UUID) for single-tenant mode
/// This matches the default organization created in migration 00001
pub const DEFAULT_ORGANIZATION_ID: &str = "00000000-0000-0000-0000-000000000001";

/// gRPC metadata key for organization ID
pub const ORGANIZATION_METADATA_KEY: &str = "x-organization-id";

/// Extracts organization_id from gRPC request metadata.
/// Falls back to DEFAULT_ORGANIZATION_ID if not provided.
pub fn get_organization_from_metadata(metadata: &MetadataMap) -> String {
    metadata
        .get(ORGANIZATION_METADATA_KEY)
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| DEFAULT_ORGANIZATION_ID.to_string())
}

/// Extracts organization_id from gRPC request.
/// Prefers AuthenticatedUser from middleware, falls back to x-organization-id header.
pub fn get_organization_from_request<T>(request: &tonic::Request<T>) -> String {
    // 1. Prefer AuthenticatedUser injected by auth middleware
    if let Some(user) = request.extensions().get::<AuthenticatedUser>() {
        return user.org_id.clone();
    }
    // 2. Fall back to header (for development/testing without auth middleware)
    get_organization_from_metadata(request.metadata())
}

/// Sets the current organization for the database session.
/// This must be called at the beginning of each request/transaction.
pub async fn set_current_organization(
    conn: &mut PgConnection,
    organization_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_current_organization($1)")
        .bind(organization_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// Gets the current organization ID from the database session.
pub async fn get_current_organization(conn: &mut PgConnection) -> Result<Option<String>, sqlx::Error> {
    let result: Option<(Option<String>,)> = sqlx::query_as("SELECT get_current_organization()")
        .fetch_optional(conn)
        .await?;
    Ok(result.and_then(|(org,)| org))
}

/// Extension trait for executing queries within an organization context.
pub trait OrganizationContext {
    /// Executes the given closure within an organization context.
    fn with_organization<'a, F, Fut, T>(
        &'a self,
        organization_id: &'a str,
        f: F,
    ) -> impl Future<Output = Result<T, sqlx::Error>> + Send + 'a
    where
        F: FnOnce(&'a PgPool) -> Fut + Send + 'a,
        Fut: Future<Output = Result<T, sqlx::Error>> + Send + 'a,
        T: Send + 'a;
}

impl OrganizationContext for PgPool {
    async fn with_organization<'a, F, Fut, T>(
        &'a self,
        organization_id: &'a str,
        f: F,
    ) -> Result<T, sqlx::Error>
    where
        F: FnOnce(&'a PgPool) -> Fut + Send + 'a,
        Fut: Future<Output = Result<T, sqlx::Error>> + Send + 'a,
        T: Send + 'a,
    {
        // Set organization context for this session
        sqlx::query("SELECT set_current_organization($1)")
            .bind(organization_id)
            .execute(self)
            .await?;

        // Execute the user's function
        f(self).await
    }
}

/// Wrapper for acquiring a connection with organization context already set.
pub struct OrganizationConnection {
    organization_id: String,
}

impl OrganizationConnection {
    pub fn new(organization_id: impl Into<String>) -> Self {
        Self {
            organization_id: organization_id.into(),
        }
    }

    /// Execute a query with organization context.
    pub async fn execute<'e, E, T, F, Fut>(
        &self,
        executor: E,
        f: F,
    ) -> Result<T, sqlx::Error>
    where
        E: Executor<'e, Database = sqlx::Postgres>,
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, sqlx::Error>>,
    {
        // First, set the organization
        sqlx::query("SELECT set_current_organization($1)")
            .bind(&self.organization_id)
            .execute(executor)
            .await?;

        // Then execute the actual query
        f().await
    }

    pub fn organization_id(&self) -> &str {
        &self.organization_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_organization_id() {
        assert_eq!(DEFAULT_ORGANIZATION_ID, "00000000-0000-0000-0000-000000000001");
    }

    #[test]
    fn test_organization_connection_new() {
        let conn = OrganizationConnection::new("test-org-uuid");
        assert_eq!(conn.organization_id(), "test-org-uuid");
    }
}
