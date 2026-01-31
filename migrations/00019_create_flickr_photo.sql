-- Create flickr_photo table for Flickr photo metadata (verified via API)
-- cam_files.flickr_id → flickr_photo.id の関係

CREATE TABLE flickr_photo (
    id TEXT NOT NULL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    secret TEXT NOT NULL,
    server TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (organization_id, id)
);

CREATE INDEX idx_flickr_photo_id ON flickr_photo(id);

ALTER TABLE flickr_photo ENABLE ROW LEVEL SECURITY;
ALTER TABLE flickr_photo FORCE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON flickr_photo
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());
