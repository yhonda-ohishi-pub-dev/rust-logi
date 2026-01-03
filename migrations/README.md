# Database Migrations

This directory contains SQL migrations for the rust-logi application.

## Running Migrations

### Using sqlx-cli

```bash
# Install sqlx-cli
cargo install sqlx-cli --no-default-features --features postgres

# Set your DATABASE_URL
export DATABASE_URL="postgres://user:password@host:5432/database"

# Run all pending migrations
sqlx migrate run

# Revert the last migration
sqlx migrate revert

# Check migration status
sqlx migrate info
```

### Using psql directly

```bash
# Run a specific migration
psql $DATABASE_URL -f migrations/00001_create_tenants.sql
```

## Multi-tenant Architecture

This application uses PostgreSQL Row Level Security (RLS) for tenant isolation.

### How RLS Works

1. Each table has a `tenant_id` column
2. RLS policies filter rows based on `app.current_tenant_id` session variable
3. Before any query, call `SELECT set_current_tenant('tenant-id')`

### Usage in Rust

```rust
use crate::db::{set_current_tenant, TenantContext, DEFAULT_TENANT_ID};

// Option 1: Set tenant manually
let mut conn = pool.acquire().await?;
set_current_tenant(&mut conn, "tenant-123").await?;
// ... execute queries ...

// Option 2: Use TenantContext trait
pool.with_tenant("tenant-123", |pool| async move {
    sqlx::query("SELECT * FROM files")
        .fetch_all(pool)
        .await
}).await?;
```

### Single-Tenant Mode

For single-tenant deployment, use the default tenant:

```rust
use crate::db::DEFAULT_TENANT_ID;

set_current_tenant(&mut conn, DEFAULT_TENANT_ID).await?;
```

## Migration Files

| File | Description |
|------|-------------|
| 00001_create_tenants.sql | Tenants table and default tenant |
| 00002_create_files.sql | Files and files_append tables |
| 00003_create_car_inspection.sql | Car inspection related tables |
| 00004_create_ichiban_cars.sql | Ichiban cars and related tables |
| 00005_create_kudguri.sql | Kudguri (運行記録) related tables |
| 00006_create_misc_tables.sql | cam_files, uriage_jisha tables |
| 00007_rls_helper_functions.sql | RLS helper functions and roles |

## Roles

- `app_user`: Normal application role (subject to RLS)
- `app_admin`: Admin role (bypasses RLS)

## Switching to Multi-tenant

When you're ready to support multiple tenants:

1. Create new tenants in the `tenants` table
2. Extract tenant ID from authentication token (JWT, etc.)
3. Call `set_current_tenant()` at the start of each request
4. All queries will automatically be scoped to that tenant
