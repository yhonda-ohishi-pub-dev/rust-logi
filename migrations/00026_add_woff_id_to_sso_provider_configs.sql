-- WOFF SDK マルチテナント対応: テナントごとの WOFF ID を保存
ALTER TABLE sso_provider_configs ADD COLUMN woff_id TEXT;
