-- Migration: Create dtakologs table for vehicle operation logs

CREATE TABLE dtakologs (
    -- 複合主キー
    data_date_time TEXT NOT NULL,
    vehicle_cd INTEGER NOT NULL,

    -- RLS用
    organization_id UUID NOT NULL REFERENCES organizations(id),

    -- 必須フィールド
    type TEXT NOT NULL,  -- __type
    all_state_font_color_index INTEGER NOT NULL DEFAULT 0,
    all_state_ryout_color TEXT NOT NULL DEFAULT 'Transparent',
    branch_cd INTEGER NOT NULL DEFAULT 0,
    branch_name TEXT NOT NULL DEFAULT '',
    current_work_cd INTEGER NOT NULL DEFAULT 0,
    data_filter_type INTEGER NOT NULL DEFAULT 0,
    disp_flag INTEGER NOT NULL DEFAULT 0,
    driver_cd INTEGER NOT NULL DEFAULT 0,
    gps_direction INTEGER NOT NULL DEFAULT 0,
    gps_enable INTEGER NOT NULL DEFAULT 0,
    gps_latitude INTEGER NOT NULL DEFAULT 0,
    gps_longitude INTEGER NOT NULL DEFAULT 0,
    gps_satellite_num INTEGER NOT NULL DEFAULT 0,
    operation_state INTEGER NOT NULL DEFAULT 0,
    recive_event_type INTEGER NOT NULL DEFAULT 0,
    recive_packet_type INTEGER NOT NULL DEFAULT 0,
    recive_work_cd INTEGER NOT NULL DEFAULT 0,
    revo INTEGER NOT NULL DEFAULT 0,
    setting_temp TEXT NOT NULL DEFAULT '',
    setting_temp1 TEXT NOT NULL DEFAULT '',
    setting_temp3 TEXT NOT NULL DEFAULT '',
    setting_temp4 TEXT NOT NULL DEFAULT '',
    speed REAL NOT NULL DEFAULT 0.0,
    sub_driver_cd INTEGER NOT NULL DEFAULT 0,
    temp_state INTEGER NOT NULL DEFAULT 0,
    vehicle_name TEXT NOT NULL DEFAULT '',

    -- オプショナルフィールド
    address_disp_c TEXT,
    address_disp_p TEXT,
    all_state TEXT,
    all_state_ex TEXT,
    all_state_font_color TEXT,
    comu_date_time TEXT,
    current_work_name TEXT,
    driver_name TEXT,
    event_val TEXT,
    gps_lati_and_long TEXT,
    odometer TEXT,
    recive_type_color_name TEXT,
    recive_type_name TEXT,
    start_work_date_time TEXT,
    state TEXT,
    state1 TEXT,
    state2 TEXT,
    state3 TEXT,
    state_flag TEXT,
    temp1 TEXT,
    temp2 TEXT,
    temp3 TEXT,
    temp4 TEXT,
    vehicle_icon_color TEXT,
    vehicle_icon_label_for_datetime TEXT,
    vehicle_icon_label_for_driver TEXT,
    vehicle_icon_label_for_vehicle TEXT,

    -- タイムスタンプ
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- 複合主キー
    PRIMARY KEY (organization_id, data_date_time, vehicle_cd)
);

-- インデックス
CREATE INDEX idx_dtakologs_organization_id ON dtakologs(organization_id);
CREATE INDEX idx_dtakologs_vehicle_cd ON dtakologs(organization_id, vehicle_cd);
CREATE INDEX idx_dtakologs_data_date_time ON dtakologs(organization_id, data_date_time DESC);
CREATE INDEX idx_dtakologs_address_disp_p ON dtakologs(organization_id, address_disp_p) WHERE address_disp_p IS NOT NULL;

-- Enable RLS
ALTER TABLE dtakologs ENABLE ROW LEVEL SECURITY;

-- RLS Policy: Organization isolation
CREATE POLICY organization_isolation_policy ON dtakologs
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- Flickr OAuth sessions table (for temporary token storage)
CREATE TABLE flickr_oauth_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    request_token TEXT NOT NULL,
    request_token_secret TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT NOW() + INTERVAL '15 minutes'
);

CREATE INDEX idx_flickr_oauth_sessions_organization ON flickr_oauth_sessions(organization_id);
CREATE INDEX idx_flickr_oauth_sessions_token ON flickr_oauth_sessions(request_token);

-- Cleanup expired sessions automatically
CREATE INDEX idx_flickr_oauth_sessions_expires ON flickr_oauth_sessions(expires_at);

ALTER TABLE flickr_oauth_sessions ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON flickr_oauth_sessions
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- Flickr tokens table (for storing access tokens)
CREATE TABLE flickr_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    access_token TEXT NOT NULL,
    access_token_secret TEXT NOT NULL,
    user_nsid TEXT NOT NULL,
    username TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(organization_id)
);

CREATE INDEX idx_flickr_tokens_organization ON flickr_tokens(organization_id);

ALTER TABLE flickr_tokens ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON flickr_tokens
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));
