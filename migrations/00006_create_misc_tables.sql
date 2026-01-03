-- Migration: Create cam_files, uriage_jisha and other misc tables with organization support

-- cam_files table (カメラファイル)
CREATE TABLE cam_files (
    name TEXT NOT NULL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    date TEXT NOT NULL,
    hour TEXT NOT NULL,
    type TEXT NOT NULL,
    cam TEXT NOT NULL,
    flickr_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (organization_id, name)
);

CREATE INDEX idx_cam_files_organization_id ON cam_files(organization_id);
CREATE INDEX idx_cam_files_date ON cam_files(organization_id, date);

ALTER TABLE cam_files ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON cam_files
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- uriage_jisha table (売上自社)
CREATE TABLE uriage_jisha (
    id SERIAL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    bumon TEXT NOT NULL,
    kingaku INTEGER,
    type INTEGER,
    date TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id),
    CONSTRAINT uriage_jisha_org_unique UNIQUE (organization_id, bumon, date)
);

CREATE INDEX idx_uriage_jisha_organization_id ON uriage_jisha(organization_id);
CREATE INDEX idx_uriage_jisha_date ON uriage_jisha(organization_id, date);

ALTER TABLE uriage_jisha ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON uriage_jisha
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));
