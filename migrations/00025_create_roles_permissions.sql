-- Role-based permission management foundation
-- Currently admin/member via user_organizations.role; this enables future granular permissions

CREATE TABLE roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT,
    is_system BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(organization_id, name)
);

CREATE TABLE permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code TEXT NOT NULL UNIQUE,
    description TEXT
);

CREATE TABLE role_permissions (
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

ALTER TABLE roles ENABLE ROW LEVEL SECURITY;
ALTER TABLE roles FORCE ROW LEVEL SECURITY;
CREATE POLICY organization_isolation_policy ON roles
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- Seed permissions
INSERT INTO permissions (code, description) VALUES
    ('org.settings.read', '組織設定の閲覧'),
    ('org.settings.edit', '組織設定の編集'),
    ('org.lineworks.manage', 'LINE WORKS設定の管理'),
    ('members.list', 'メンバー一覧の閲覧'),
    ('members.invite', 'メンバーの招待'),
    ('members.remove', 'メンバーの削除'),
    ('members.role.edit', 'メンバーのロール変更');
