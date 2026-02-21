-- Bot configurations per organization
-- Supports multiple bots per org (e.g., multiple LINE WORKS bots)

CREATE TABLE bot_configs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    provider TEXT NOT NULL DEFAULT 'lineworks',
    name TEXT NOT NULL,                         -- display name (e.g., "大石運輸 Bot")
    client_id TEXT NOT NULL,
    client_secret_encrypted TEXT NOT NULL,       -- AES-256-GCM encrypted
    service_account TEXT NOT NULL,               -- JWT sub claim
    private_key_encrypted TEXT NOT NULL,         -- AES-256-GCM encrypted (PEM)
    bot_id TEXT NOT NULL,                        -- LINE WORKS bot ID
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- One bot_id is globally unique per provider
CREATE UNIQUE INDEX idx_bot_configs_provider_bot_id ON bot_configs(provider, bot_id);
-- Fast lookup by org
CREATE INDEX idx_bot_configs_org ON bot_configs(organization_id) WHERE enabled = TRUE;

-- RLS: same pattern as sso_provider_configs (enabled but not forced for admin access)
ALTER TABLE bot_configs ENABLE ROW LEVEL SECURITY;
CREATE POLICY organization_isolation_policy ON bot_configs
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- FORCE RLS for non-superuser queries
ALTER TABLE bot_configs FORCE ROW LEVEL SECURITY;
