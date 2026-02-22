-- Access requests: OAuth アカウントによる組織参加リクエスト
CREATE TABLE access_requests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    user_id UUID NOT NULL REFERENCES app_users(id),
    email TEXT NOT NULL,
    display_name TEXT NOT NULL DEFAULT '',
    avatar_url TEXT,
    provider TEXT NOT NULL DEFAULT '',  -- 'google', 'lineworks', etc.
    status TEXT NOT NULL DEFAULT 'pending',  -- pending / approved / declined
    role TEXT,  -- 承認時に設定: 'admin' or 'member'
    reviewed_by UUID REFERENCES app_users(id),
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 同一ユーザー・同一組織で pending は1件のみ
CREATE UNIQUE INDEX idx_access_requests_pending
    ON access_requests(user_id, organization_id) WHERE status = 'pending';

CREATE INDEX idx_access_requests_org_status ON access_requests(organization_id, status);

-- RLS: ENABLE（NOT FORCE — リクエスト者はまだ組織メンバーではないため）
ALTER TABLE access_requests ENABLE ROW LEVEL SECURITY;

CREATE POLICY access_requests_select ON access_requests
    FOR SELECT USING (organization_id = get_current_organization_uuid());

CREATE POLICY access_requests_update ON access_requests
    FOR UPDATE USING (organization_id = get_current_organization_uuid());

CREATE POLICY access_requests_insert ON access_requests
    FOR INSERT WITH CHECK (true);
