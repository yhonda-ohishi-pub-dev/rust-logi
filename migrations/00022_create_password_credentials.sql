-- Migration: Create password_credentials table for invited users with password login
-- Links to app_users (unified identity), org-scoped username

CREATE TABLE password_credentials (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    app_user_id UUID NOT NULL REFERENCES app_users(id) ON DELETE CASCADE,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    username TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(organization_id, username)
);

-- RLS: org isolation policy
ALTER TABLE password_credentials ENABLE ROW LEVEL SECURITY;
-- NO FORCE RLS: login needs to query by username without org context (same as api_users)
CREATE POLICY organization_isolation_policy ON password_credentials
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

CREATE INDEX idx_password_credentials_app_user_id ON password_credentials(app_user_id);
CREATE INDEX idx_password_credentials_org_username ON password_credentials(organization_id, username);
