-- Migration: Create items table with dual ownership (org + personal), hierarchy, and barcode support

-- ============================================================
-- 1. user_id 用 RLS ヘルパー関数
-- ============================================================

-- セッション変数に user_id を設定
CREATE OR REPLACE FUNCTION set_current_user_id(p_user_id TEXT)
RETURNS VOID AS $$
BEGIN
    PERFORM set_config('app.current_user_id', p_user_id, false);
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- セッション変数から user_id を UUID で取得
CREATE OR REPLACE FUNCTION get_current_user_uuid() RETURNS UUID AS $$
BEGIN
    RETURN current_setting('app.current_user_id', true)::uuid;
EXCEPTION
    WHEN OTHERS THEN
        RETURN NULL;
END;
$$ LANGUAGE plpgsql STABLE;

COMMENT ON FUNCTION set_current_user_id(TEXT) IS
'Sets the current user for personal items RLS policies. Call this at the beginning of each request that accesses personal data.
Example: SELECT set_current_user_id(''550e8400-e29b-41d4-a716-446655440000'');';

COMMENT ON FUNCTION get_current_user_uuid() IS
'Returns the current user ID set in the session as UUID. Returns NULL if not set.';

-- ============================================================
-- 2. items テーブル
-- ============================================================

CREATE TABLE items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    parent_id UUID REFERENCES items(id) ON DELETE SET NULL,
    owner_type TEXT NOT NULL DEFAULT 'org',
    organization_id UUID REFERENCES organizations(id),
    user_id UUID REFERENCES app_users(id),
    name TEXT NOT NULL,
    barcode TEXT,
    category TEXT,
    description TEXT,
    image_url TEXT,
    quantity INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT items_owner_check CHECK (
        (owner_type = 'org' AND organization_id IS NOT NULL) OR
        (owner_type = 'personal' AND user_id IS NOT NULL)
    )
);

-- ============================================================
-- 3. RLS ポリシー (PERMISSIVE: org と personal が OR で結合)
-- ============================================================

ALTER TABLE items ENABLE ROW LEVEL SECURITY;
ALTER TABLE items FORCE ROW LEVEL SECURITY;

-- 組織物品: get_current_organization_uuid() を使用（既存パターン踏襲）
CREATE POLICY items_org_policy ON items
    FOR ALL USING (
        owner_type = 'org'
        AND organization_id = get_current_organization_uuid()
    )
    WITH CHECK (
        owner_type = 'org'
        AND organization_id = get_current_organization_uuid()
    );

-- 個人物品: get_current_user_uuid() を使用
CREATE POLICY items_personal_policy ON items
    FOR ALL USING (
        owner_type = 'personal'
        AND user_id = get_current_user_uuid()
    )
    WITH CHECK (
        owner_type = 'personal'
        AND user_id = get_current_user_uuid()
    );

-- ============================================================
-- 4. インデックス
-- ============================================================

CREATE INDEX idx_items_org ON items(organization_id) WHERE owner_type = 'org';
CREATE INDEX idx_items_user ON items(user_id) WHERE owner_type = 'personal';
CREATE INDEX idx_items_parent ON items(parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX idx_items_barcode ON items(barcode) WHERE barcode IS NOT NULL;
CREATE INDEX idx_items_category ON items(category) WHERE category IS NOT NULL;
