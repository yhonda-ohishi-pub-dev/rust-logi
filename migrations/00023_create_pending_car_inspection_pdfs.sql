-- Migration: Create pending_car_inspection_pdfs table
-- PDFが先にアップロードされた場合の一時保存用テーブル
-- JSONアップロード時にこのテーブルを検索し、マッチするPDFがあればcar_inspection_files_bにリンク

CREATE TABLE pending_car_inspection_pdfs (
    id SERIAL PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    file_uuid UUID NOT NULL,
    "ElectCertMgNo" TEXT NOT NULL,
    "GrantdateE" TEXT NOT NULL,
    "GrantdateY" TEXT NOT NULL,
    "GrantdateM" TEXT NOT NULL,
    "GrantdateD" TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT pending_pdf_unique UNIQUE (organization_id, "ElectCertMgNo")
);

CREATE INDEX idx_pending_pdf_org_ecmn ON pending_car_inspection_pdfs(organization_id, "ElectCertMgNo");

ALTER TABLE pending_car_inspection_pdfs ENABLE ROW LEVEL SECURITY;
ALTER TABLE pending_car_inspection_pdfs FORCE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON pending_car_inspection_pdfs
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));
