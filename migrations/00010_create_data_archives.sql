-- Data archives table (tracks archived data per parent record)
CREATE TABLE data_archives (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),

    -- Archive target (method_id defined in Rust code)
    method_id TEXT NOT NULL,                  -- 'kudguri', 'orders', etc.
    parent_pk_uuid UUID,
    parent_pk_bigint BIGINT,
    parent_pk_text TEXT,

    -- Storage info
    storage_type TEXT NOT NULL DEFAULT 'gcs',
    storage_base_path TEXT NOT NULL,          -- 'org-uuid/kudguri/parent-uuid/'
    storage_class TEXT,

    -- File information
    files JSONB NOT NULL DEFAULT '[]',        -- [{"file": "ivt.json", "checksum": "...", "byte_size": 123, "record_count": 45}, ...]

    -- Lifecycle
    status TEXT NOT NULL DEFAULT 'scheduled', -- scheduled, pending, archived, verified, restored, deleted
    scheduled_at TIMESTAMPTZ NOT NULL,
    archived_at TIMESTAMPTZ,
    verified_at TIMESTAMPTZ,
    source_deleted_at TIMESTAMPTZ,
    restored_at TIMESTAMPTZ,

    -- Metadata
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Unique constraints (method + parent PK)
CREATE UNIQUE INDEX idx_data_archives_method_uuid
  ON data_archives(method_id, parent_pk_uuid)
  WHERE parent_pk_uuid IS NOT NULL;

CREATE UNIQUE INDEX idx_data_archives_method_bigint
  ON data_archives(method_id, parent_pk_bigint)
  WHERE parent_pk_bigint IS NOT NULL;

CREATE UNIQUE INDEX idx_data_archives_method_text
  ON data_archives(method_id, parent_pk_text)
  WHERE parent_pk_text IS NOT NULL;

-- Search indexes
CREATE INDEX idx_data_archives_org ON data_archives(organization_id);
CREATE INDEX idx_data_archives_status ON data_archives(status);
CREATE INDEX idx_data_archives_scheduled ON data_archives(scheduled_at) WHERE status = 'scheduled';

-- Status transition logs (for auditing)
CREATE TABLE data_archive_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    archive_id UUID NOT NULL REFERENCES data_archives(id),
    action TEXT NOT NULL,                     -- 'archive', 'verify', 'delete_source', 'restore', 're_archive'
    old_status TEXT,
    new_status TEXT NOT NULL,
    details JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX idx_data_archive_logs_archive ON data_archive_logs(archive_id);

-- Enable RLS
ALTER TABLE data_archives ENABLE ROW LEVEL SECURITY;
ALTER TABLE data_archive_logs ENABLE ROW LEVEL SECURITY;

-- RLS policies for data_archives
CREATE POLICY data_archives_select ON data_archives
    FOR SELECT USING (organization_id::text = current_setting('app.current_organization_id', true));

CREATE POLICY data_archives_insert ON data_archives
    FOR INSERT WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

CREATE POLICY data_archives_update ON data_archives
    FOR UPDATE USING (organization_id::text = current_setting('app.current_organization_id', true));

CREATE POLICY data_archives_delete ON data_archives
    FOR DELETE USING (organization_id::text = current_setting('app.current_organization_id', true));

-- RLS policies for data_archive_logs (via archive_id join)
CREATE POLICY data_archive_logs_select ON data_archive_logs
    FOR SELECT USING (
        EXISTS (
            SELECT 1 FROM data_archives da
            WHERE da.id = archive_id
            AND da.organization_id::text = current_setting('app.current_organization_id', true)
        )
    );

CREATE POLICY data_archive_logs_insert ON data_archive_logs
    FOR INSERT WITH CHECK (
        EXISTS (
            SELECT 1 FROM data_archives da
            WHERE da.id = archive_id
            AND da.organization_id::text = current_setting('app.current_organization_id', true)
        )
    );
