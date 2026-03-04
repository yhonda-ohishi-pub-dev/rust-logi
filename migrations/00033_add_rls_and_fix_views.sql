-- Migration: Add RLS to remaining auth tables and fix SECURITY DEFINER views
-- Fixes Supabase security linter errors:
--   - rls_disabled_in_public: organizations, app_users, user_organizations,
--     oauth_accounts, invitations, api_tokens, permissions, role_permissions
--   - security_definer_view: daily_access_stats, file_access_stats
--
-- Note: Backend connects as app_admin (BYPASSRLS) so no behavioral change on backend.
--       Only PostgREST (app_user role) access is restricted.
-- Note: _sqlx_migrations is skipped — enabling RLS on it would risk breaking
--       sqlx's migration tracking at startup.

-- ========================================
-- 1. 組織スコープ テーブル (RLS + ポリシー)
-- ========================================

-- organizations: id が現在の組織と一致する行のみ参照可能
ALTER TABLE organizations ENABLE ROW LEVEL SECURITY;

CREATE POLICY org_isolation ON organizations
    FOR ALL
    USING (id = get_current_organization_uuid());

-- user_organizations: 現在の組織のメンバーシップのみ参照可能
ALTER TABLE user_organizations ENABLE ROW LEVEL SECURITY;

CREATE POLICY org_isolation ON user_organizations
    FOR ALL
    USING (organization_id = get_current_organization_uuid());

-- invitations: 現在の組織の招待のみ参照可能
ALTER TABLE invitations ENABLE ROW LEVEL SECURITY;

CREATE POLICY org_isolation ON invitations
    FOR ALL
    USING (organization_id = get_current_organization_uuid());

-- ========================================
-- 2. ユーザースコープ テーブル (RLS のみ、DENY ALL)
-- セッション変数に user_id がないため app_user からのアクセスを全拒否。
-- app_admin (BYPASSRLS) 経由のバックエンドは影響なし。
-- ========================================

ALTER TABLE app_users ENABLE ROW LEVEL SECURITY;

ALTER TABLE oauth_accounts ENABLE ROW LEVEL SECURITY;

ALTER TABLE api_tokens ENABLE ROW LEVEL SECURITY;

-- ========================================
-- 3. グローバル参照テーブル (SELECT のみ許可)
-- ========================================

-- permissions: 権限コードは全ユーザーが読み取り可能な参照データ
ALTER TABLE permissions ENABLE ROW LEVEL SECURITY;

CREATE POLICY allow_select ON permissions
    FOR SELECT
    USING (TRUE);

-- role_permissions: ロールと権限の紐づけも読み取り可能
ALTER TABLE role_permissions ENABLE ROW LEVEL SECURITY;

CREATE POLICY allow_select ON role_permissions
    FOR SELECT
    USING (TRUE);

-- ========================================
-- 4. SECURITY DEFINER ビューの修正
-- PostgreSQL 15+ の security_invoker = true で再作成。
-- 呼び出し元のロール権限と RLS を尊重して実行されるようになる。
-- ========================================

CREATE OR REPLACE VIEW daily_access_stats WITH (security_invoker = true) AS
SELECT
    DATE(accessed_at) as access_date,
    storage_class_at_access,
    COUNT(*) as access_count,
    COUNT(DISTINCT file_uuid) as unique_files
FROM file_access_logs
WHERE accessed_at > NOW() - INTERVAL '30 days'
GROUP BY DATE(accessed_at), storage_class_at_access
ORDER BY access_date DESC;

CREATE OR REPLACE VIEW file_access_stats WITH (security_invoker = true) AS
SELECT
    f.uuid,
    f.filename,
    f.storage_class,
    f.access_count_weekly,
    f.access_count_total,
    f.last_accessed_at,
    f.promoted_to_standard_at,
    get_recent_access_count(f.uuid, 7) as access_count_7days,
    get_recent_access_count(f.uuid, 30) as access_count_30days
FROM files f
WHERE f.deleted_at IS NULL AND f.s3_key IS NOT NULL;
