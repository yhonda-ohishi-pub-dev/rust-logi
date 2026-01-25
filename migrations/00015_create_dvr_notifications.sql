-- Create DVR notifications table

CREATE TABLE dvr_notifications (
    mp4_url TEXT NOT NULL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    vehicle_cd BIGINT NOT NULL,
    vehicle_name TEXT NOT NULL,
    serial_no TEXT NOT NULL,
    file_name TEXT NOT NULL,
    event_type TEXT NOT NULL,
    dvr_datetime TEXT NOT NULL,
    driver_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (organization_id, mp4_url)
);

-- Performance indexes
CREATE INDEX idx_dvr_notifications_vehicle_cd ON dvr_notifications(organization_id, vehicle_cd);
CREATE INDEX idx_dvr_notifications_dvr_datetime ON dvr_notifications(organization_id, dvr_datetime DESC);

-- Row Level Security
ALTER TABLE dvr_notifications ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON dvr_notifications
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));
