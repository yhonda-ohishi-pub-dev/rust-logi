-- Migration: Create ichiban_cars related tables with organization support

-- Main ichiban_cars table
CREATE TABLE ichiban_cars (
    id TEXT NOT NULL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    id4 TEXT NOT NULL,
    name TEXT,
    name_r TEXT,
    shashu TEXT NOT NULL,
    sekisai NUMERIC,
    reg_date TEXT,
    parch_date TEXT,
    scrap_date TEXT,
    bumon_code_id TEXT,
    driver_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    modified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (organization_id, id)
);

CREATE INDEX idx_ichiban_cars_organization_id ON ichiban_cars(organization_id);

ALTER TABLE ichiban_cars ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON ichiban_cars
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- dtako_cars_ichiban_cars mapping table
CREATE TABLE dtako_cars_ichiban_cars (
    id_dtako TEXT NOT NULL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (organization_id, id_dtako)
);

CREATE INDEX idx_dtako_cars_organization_id ON dtako_cars_ichiban_cars(organization_id);

ALTER TABLE dtako_cars_ichiban_cars ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON dtako_cars_ichiban_cars
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- car_ins_sheet_ichiban_cars mapping table
CREATE TABLE car_ins_sheet_ichiban_cars (
    id SERIAL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    id_cars TEXT,
    "ElectCertMgNo" TEXT NOT NULL,
    "ElectCertPublishdateE" TEXT NOT NULL,
    "ElectCertPublishdateY" TEXT NOT NULL,
    "ElectCertPublishdateM" TEXT NOT NULL,
    "ElectCertPublishdateD" TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id),
    CONSTRAINT car_ins_sheet_ichiban_unique UNIQUE (
        organization_id, "ElectCertMgNo", "ElectCertPublishdateE",
        "ElectCertPublishdateY", "ElectCertPublishdateM", "ElectCertPublishdateD"
    )
);

CREATE INDEX idx_car_ins_sheet_organization_id ON car_ins_sheet_ichiban_cars(organization_id);

ALTER TABLE car_ins_sheet_ichiban_cars ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON car_ins_sheet_ichiban_cars
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- car_ins_sheet_ichiban_cars_a (alternative key structure)
CREATE TABLE car_ins_sheet_ichiban_cars_a (
    id SERIAL,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    id_cars TEXT,
    "ElectCertMgNo" TEXT NOT NULL,
    "GrantdateE" TEXT NOT NULL,
    "GrantdateY" TEXT NOT NULL,
    "GrantdateM" TEXT NOT NULL,
    "GrantdateD" TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id),
    CONSTRAINT car_ins_sheet_ichiban_a_unique UNIQUE (
        organization_id, "ElectCertMgNo", "GrantdateE",
        "GrantdateY", "GrantdateM", "GrantdateD"
    )
);

CREATE INDEX idx_car_ins_sheet_a_organization_id ON car_ins_sheet_ichiban_cars_a(organization_id);

ALTER TABLE car_ins_sheet_ichiban_cars_a ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON car_ins_sheet_ichiban_cars_a
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- car_inspection_files_a
CREATE TABLE car_inspection_files_a (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    type TEXT NOT NULL,
    "ElectCertMgNo" TEXT NOT NULL,
    "GrantdateE" TEXT NOT NULL,
    "GrantdateY" TEXT NOT NULL,
    "GrantdateM" TEXT NOT NULL,
    "GrantdateD" TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    modified_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_car_inspection_files_a_organization_id ON car_inspection_files_a(organization_id);

ALTER TABLE car_inspection_files_a ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON car_inspection_files_a
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- car_inspection_files_b
CREATE TABLE car_inspection_files_b (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    type TEXT NOT NULL,
    "ElectCertMgNo" TEXT NOT NULL,
    "GrantdateE" TEXT NOT NULL,
    "GrantdateY" TEXT NOT NULL,
    "GrantdateM" TEXT NOT NULL,
    "GrantdateD" TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    modified_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_car_inspection_files_b_organization_id ON car_inspection_files_b(organization_id);

ALTER TABLE car_inspection_files_b ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON car_inspection_files_b
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- stuff table
CREATE TABLE stuff (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    user_id UUID NOT NULL,
    idp_id TEXT NOT NULL,
    type TEXT DEFAULT 'stuff',
    name TEXT NOT NULL,
    file_uuid UUID,
    parent_uuid UUID,
    description TEXT DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_stuff_organization_id ON stuff(organization_id);
CREATE INDEX idx_stuff_user ON stuff(organization_id, user_id);

ALTER TABLE stuff ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON stuff
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));
