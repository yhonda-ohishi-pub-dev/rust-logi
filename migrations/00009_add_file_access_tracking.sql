-- Migration: Add file access tracking for smart storage class management

-- ========================================
-- 1. アクセスログテーブル（詳細統計用）
-- ========================================
CREATE TABLE file_access_logs (
    id BIGSERIAL PRIMARY KEY,
    file_uuid UUID NOT NULL REFERENCES files(uuid) ON DELETE CASCADE,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    accessed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- アクセス時のストレージクラス（分析用）
    storage_class_at_access TEXT
);

-- 検索用インデックス
CREATE INDEX idx_file_access_logs_file_uuid ON file_access_logs(file_uuid);
CREATE INDEX idx_file_access_logs_accessed_at ON file_access_logs(accessed_at);
CREATE INDEX idx_file_access_logs_org_accessed ON file_access_logs(organization_id, accessed_at);

-- RLSポリシー
ALTER TABLE file_access_logs ENABLE ROW LEVEL SECURITY;

CREATE POLICY file_access_logs_org_isolation ON file_access_logs
    USING (organization_id::text = get_current_organization());

-- ========================================
-- 2. filesテーブルにアクセスカウント追加（高速判定用）
-- ========================================

-- 週間アクセスカウント（STANDARD化判定用）
ALTER TABLE files ADD COLUMN access_count_weekly INT NOT NULL DEFAULT 0;

-- 週カウント開始日
ALTER TABLE files ADD COLUMN week_started_at TIMESTAMPTZ;

-- 累計アクセスカウント（統計用）
ALTER TABLE files ADD COLUMN access_count_total INT NOT NULL DEFAULT 0;

-- STANDARDに昇格した日時（早期削除ペナルティ回避の判定用）
ALTER TABLE files ADD COLUMN promoted_to_standard_at TIMESTAMPTZ;

-- ========================================
-- 3. 便利な関数
-- ========================================

-- 直近7日間のアクセス数を取得
CREATE OR REPLACE FUNCTION get_recent_access_count(p_file_uuid UUID, p_days INT DEFAULT 7)
RETURNS INT AS $$
BEGIN
    RETURN (
        SELECT COUNT(*)::INT
        FROM file_access_logs
        WHERE file_uuid = p_file_uuid
          AND accessed_at > NOW() - make_interval(days => p_days)
    );
END;
$$ LANGUAGE plpgsql STABLE;

-- アクセスを記録し、週間カウントを更新（1つのトランザクションで）
CREATE OR REPLACE FUNCTION record_file_access(
    p_file_uuid UUID,
    p_organization_id UUID,
    p_storage_class TEXT DEFAULT NULL
)
RETURNS TABLE(
    weekly_count INT,
    total_count INT,
    recent_7day_count INT
) AS $$
DECLARE
    v_week_started TIMESTAMPTZ;
    v_new_weekly INT;
    v_new_total INT;
BEGIN
    -- アクセスログに記録
    INSERT INTO file_access_logs (file_uuid, organization_id, storage_class_at_access)
    VALUES (p_file_uuid, p_organization_id, p_storage_class);

    -- filesテーブルの週間カウントを更新
    SELECT week_started_at INTO v_week_started
    FROM files WHERE uuid = p_file_uuid;

    -- 週が変わっていたらリセット
    IF v_week_started IS NULL OR v_week_started < NOW() - INTERVAL '7 days' THEN
        UPDATE files
        SET access_count_weekly = 1,
            access_count_total = access_count_total + 1,
            week_started_at = NOW(),
            last_accessed_at = NOW()
        WHERE uuid = p_file_uuid
        RETURNING access_count_weekly, access_count_total
        INTO v_new_weekly, v_new_total;
    ELSE
        UPDATE files
        SET access_count_weekly = access_count_weekly + 1,
            access_count_total = access_count_total + 1,
            last_accessed_at = NOW()
        WHERE uuid = p_file_uuid
        RETURNING access_count_weekly, access_count_total
        INTO v_new_weekly, v_new_total;
    END IF;

    RETURN QUERY SELECT
        v_new_weekly,
        v_new_total,
        get_recent_access_count(p_file_uuid, 7);
END;
$$ LANGUAGE plpgsql;

-- ========================================
-- 4. 統計ビュー（分析用）
-- ========================================

-- ファイルごとのアクセス統計
CREATE OR REPLACE VIEW file_access_stats AS
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

-- 日別アクセス統計
CREATE OR REPLACE VIEW daily_access_stats AS
SELECT
    DATE(accessed_at) as access_date,
    storage_class_at_access,
    COUNT(*) as access_count,
    COUNT(DISTINCT file_uuid) as unique_files
FROM file_access_logs
WHERE accessed_at > NOW() - INTERVAL '30 days'
GROUP BY DATE(accessed_at), storage_class_at_access
ORDER BY access_date DESC;
