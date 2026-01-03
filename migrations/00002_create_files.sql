-- Migration: Create files table with organization support

CREATE TABLE files (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    filename TEXT NOT NULL,
    type TEXT NOT NULL,
    blob TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

-- Index for organization queries
CREATE INDEX idx_files_organization_id ON files(organization_id);

-- Index for soft delete queries within organization
CREATE INDEX idx_files_org_deleted ON files(organization_id, deleted_at) WHERE deleted_at IS NULL;

-- Enable RLS
ALTER TABLE files ENABLE ROW LEVEL SECURITY;

-- RLS Policy: Organization isolation
CREATE POLICY organization_isolation_policy ON files
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- files_append table
CREATE TABLE files_append (
    appendname TEXT PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    file_uuid UUID NOT NULL REFERENCES files(uuid),
    appendtype TEXT NOT NULL,
    type TEXT NOT NULL,
    page INTEGER DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_files_append_organization_id ON files_append(organization_id);
CREATE INDEX idx_files_append_file_uuid ON files_append(file_uuid);

ALTER TABLE files_append ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON files_append
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));
