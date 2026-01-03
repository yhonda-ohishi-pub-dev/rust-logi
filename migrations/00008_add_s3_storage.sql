-- Migration: Add S3 storage support to files table

-- S3キー（S3上のオブジェクトパス）
ALTER TABLE files ADD COLUMN s3_key TEXT;

-- ストレージクラス（STANDARD, STANDARD_IA, GLACIER等）
ALTER TABLE files ADD COLUMN storage_class TEXT DEFAULT 'STANDARD';

-- 最終アクセス日時（アクセス時にStandardに戻す判定用）
ALTER TABLE files ADD COLUMN last_accessed_at TIMESTAMPTZ;

-- S3キーのインデックス
CREATE INDEX idx_files_s3_key ON files(s3_key) WHERE s3_key IS NOT NULL;

-- 最終アクセス日時のインデックス（古いファイルの検索用）
CREATE INDEX idx_files_last_accessed ON files(last_accessed_at);

-- コメント: s3_keyがNULLの場合は既存のblobカラムを使用（後方互換）
-- 移行完了後、blobカラムは削除可能
