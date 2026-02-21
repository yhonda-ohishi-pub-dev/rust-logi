-- SSO provider configurations per organization
-- Supports multiple providers: lineworks, discord, slack, etc.

CREATE TABLE sso_provider_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,                   -- 'lineworks', 'discord', 'slack'
    client_id TEXT NOT NULL,
    client_secret_encrypted TEXT NOT NULL,
    external_org_id TEXT NOT NULL,            -- domain for LW, guild_id for Discord, workspace for Slack
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(organization_id, provider),
    UNIQUE(provider, external_org_id),
    UNIQUE(provider, client_id)
);

CREATE INDEX idx_sso_provider_configs_lookup ON sso_provider_configs(provider, external_org_id)
    WHERE enabled = TRUE;

-- RLS enabled but NOT forced: auth flow needs to query without org context
-- Same pattern as password_credentials (00022)
ALTER TABLE sso_provider_configs ENABLE ROW LEVEL SECURITY;
CREATE POLICY organization_isolation_policy ON sso_provider_configs
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());
