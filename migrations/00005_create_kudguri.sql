-- Migration: Create kudguri related tables with organization support

-- Main kudguri table (運行記録)
CREATE TABLE kudguri (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    hash TEXT NOT NULL,
    unkou_no TEXT NOT NULL,
    kudguri_uuid TEXT NOT NULL,
    read_date TEXT,
    office_cd TEXT,
    office_name TEXT,
    vehicle_cd TEXT,
    vehicle_name TEXT,
    driver_cd1 TEXT,
    driver_name1 TEXT,
    target_driver_type TEXT NOT NULL,
    target_driver_cd TEXT,
    target_driver_name TEXT,
    start_datetime TEXT,
    end_datetime TEXT,
    event_cd TEXT,
    event_name TEXT,
    start_mileage TEXT,
    end_mileage TEXT,
    section_time TEXT,
    section_distance TEXT,
    start_city_cd TEXT,
    start_city_name TEXT,
    end_city_cd TEXT,
    end_city_name TEXT,
    start_place_cd TEXT,
    start_place_name TEXT,
    end_place_cd TEXT,
    end_place_name TEXT,
    start_gps_valid TEXT,
    start_gps_lat TEXT,
    start_gps_lng TEXT,
    end_gps_valid TEXT,
    end_gps_lat TEXT,
    end_gps_lng TEXT,
    over_limit_max TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_kudguri_organization_id ON kudguri(organization_id);
CREATE INDEX idx_kudguri_unkou ON kudguri(organization_id, unkou_no);

ALTER TABLE kudguri ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON kudguri
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- kudgcst table (高速代)
CREATE TABLE kudgcst (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    hash TEXT NOT NULL,
    kudguri_uuid UUID REFERENCES kudguri(uuid),
    unkou_no TEXT,
    unkou_date TEXT,
    read_date TEXT,
    office_cd TEXT,
    office_name TEXT,
    vehicle_cd TEXT,
    vehicle_name TEXT,
    driver_cd1 TEXT,
    driver_name1 TEXT,
    target_driver_type TEXT NOT NULL,
    start_datetime TEXT,
    end_datetime TEXT,
    ferry_company_cd TEXT,
    ferry_company_name TEXT,
    boarding_place_cd TEXT,
    boarding_place_name TEXT,
    trip_number TEXT,
    dropoff_place_cd TEXT,
    dropoff_place_name TEXT,
    settlement_type TEXT,
    settlement_type_name TEXT,
    standard_fare TEXT,
    contract_fare TEXT,
    ferry_vehicle_type TEXT,
    ferry_vehicle_type_name TEXT,
    assumed_distance TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_kudgcst_organization_id ON kudgcst(organization_id);
CREATE INDEX idx_kudgcst_kudguri ON kudgcst(kudguri_uuid);

ALTER TABLE kudgcst ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON kudgcst
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- kudgfry table (給油・点検)
CREATE TABLE kudgfry (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    hash TEXT NOT NULL,
    kudguri_uuid UUID REFERENCES kudguri(uuid),
    target_driver_type TEXT NOT NULL,
    unkou_no TEXT,
    unkou_date TEXT,
    read_date TEXT,
    office_cd TEXT,
    office_name TEXT,
    vehicle_cd TEXT,
    vehicle_name TEXT,
    driver_cd1 TEXT,
    driver_name1 TEXT,
    driver_cd2 TEXT,
    driver_name2 TEXT,
    relevant_datetime TEXT,
    refuel_inspect_category TEXT,
    refuel_inspect_category_name TEXT,
    refuel_inspect_type TEXT,
    refuel_inspect_type_name TEXT,
    refuel_inspect_kind TEXT,
    refuel_inspect_kind_name TEXT,
    refill_amount TEXT,
    own_other_type TEXT,
    mileage TEXT,
    meter_value TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_kudgfry_organization_id ON kudgfry(organization_id);
CREATE INDEX idx_kudgfry_kudguri ON kudgfry(kudguri_uuid);

ALTER TABLE kudgfry ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON kudgfry
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- kudgful table (走行区間)
CREATE TABLE kudgful (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    hash TEXT NOT NULL,
    kudguri_uuid UUID REFERENCES kudguri(uuid),
    unkou_no TEXT,
    read_date TEXT,
    office_cd TEXT,
    office_name TEXT,
    vehicle_cd TEXT,
    vehicle_name TEXT,
    driver_cd1 TEXT,
    driver_name1 TEXT,
    target_driver_type TEXT NOT NULL,
    target_driver_cd TEXT,
    target_driver_name TEXT,
    start_datetime TEXT,
    end_datetime TEXT,
    event_cd TEXT,
    event_name TEXT,
    start_mileage TEXT,
    end_mileage TEXT,
    section_time TEXT,
    section_distance TEXT,
    start_city_cd TEXT,
    start_city_name TEXT,
    end_city_cd TEXT,
    end_city_name TEXT,
    start_place_cd TEXT,
    start_place_name TEXT,
    end_place_cd TEXT,
    end_place_name TEXT,
    start_gps_valid TEXT,
    start_gps_lat TEXT,
    start_gps_lng TEXT,
    end_gps_valid TEXT,
    end_gps_lat TEXT,
    end_gps_lng TEXT,
    over_limit_max TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_kudgful_organization_id ON kudgful(organization_id);
CREATE INDEX idx_kudgful_kudguri ON kudgful(kudguri_uuid);

ALTER TABLE kudgful ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON kudgful
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- kudgivt table (運行日報)
CREATE TABLE kudgivt (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    hash TEXT NOT NULL,
    kudguri_uuid UUID REFERENCES kudguri(uuid),
    unkou_no TEXT,
    read_date TEXT,
    unkou_date TEXT,
    office_cd TEXT,
    office_name TEXT,
    vehicle_cd TEXT,
    vehicle_name TEXT,
    driver_cd1 TEXT,
    driver_name1 TEXT,
    target_driver_type TEXT NOT NULL,
    target_driver_cd TEXT,
    target_driver_name TEXT,
    clock_in_datetime TEXT,
    clock_out_datetime TEXT,
    departure_datetime TEXT,
    return_datetime TEXT,
    departure_meter TEXT,
    return_meter TEXT,
    total_mileage TEXT,
    destination_city_name TEXT,
    destination_place_name TEXT,
    actual_mileage TEXT,
    local_drive_time TEXT,
    express_drive_time TEXT,
    bypass_drive_time TEXT,
    actual_drive_time TEXT,
    empty_drive_time TEXT,
    work1_time TEXT,
    work2_time TEXT,
    work3_time TEXT,
    work4_time TEXT,
    work5_time TEXT,
    work6_time TEXT,
    work7_time TEXT,
    work8_time TEXT,
    work9_time TEXT,
    work10_time TEXT,
    state1_distance TEXT,
    state1_time TEXT,
    state2_distance TEXT,
    state2_time TEXT,
    state3_distance TEXT,
    state3_time TEXT,
    state4_distance TEXT,
    state4_time TEXT,
    state5_distance TEXT,
    state5_time TEXT,
    own_main_fuel TEXT,
    own_main_additive TEXT,
    own_consumable TEXT,
    other_main_fuel TEXT,
    other_main_additive TEXT,
    other_consumable TEXT,
    local_speed_over_max TEXT,
    local_speed_over_time TEXT,
    local_speed_over_count TEXT,
    express_speed_over_max TEXT,
    express_speed_over_time TEXT,
    express_speed_over_count TEXT,
    dedicated_speed_over_max TEXT,
    dedicated_speed_over_time TEXT,
    dedicated_speed_over_count TEXT,
    idling_time TEXT,
    idling_time_count TEXT,
    rotation_over_max TEXT,
    rotation_over_count TEXT,
    rotation_over_time TEXT,
    rapid_accel_count1 TEXT,
    rapid_accel_count2 TEXT,
    rapid_accel_count3 TEXT,
    rapid_accel_count4 TEXT,
    rapid_accel_count5 TEXT,
    rapid_accel_max TEXT,
    rapid_accel_max_speed TEXT,
    rapid_decel_count1 TEXT,
    rapid_decel_count2 TEXT,
    rapid_decel_count3 TEXT,
    rapid_decel_count4 TEXT,
    rapid_decel_count5 TEXT,
    rapid_decel_max TEXT,
    rapid_decel_max_speed TEXT,
    rapid_curve_count1 TEXT,
    rapid_curve_count2 TEXT,
    rapid_curve_count3 TEXT,
    rapid_curve_count4 TEXT,
    rapid_curve_count5 TEXT,
    rapid_curve_max TEXT,
    rapid_curve_max_speed TEXT,
    continuous_drive_over_count TEXT,
    continuous_drive_max_time TEXT,
    continuous_drive_total_time TEXT,
    wave_drive_count TEXT,
    wave_drive_max_time TEXT,
    wave_drive_max_speed_diff TEXT,
    local_speed_score TEXT,
    express_speed_score TEXT,
    dedicated_speed_score TEXT,
    local_distance_score TEXT,
    express_distance_score TEXT,
    dedicated_distance_score TEXT,
    rapid_accel_score TEXT,
    rapid_decel_score TEXT,
    rapid_curve_score TEXT,
    actual_low_speed_rotation_score TEXT,
    actual_high_speed_rotation_score TEXT,
    empty_low_speed_rotation_score TEXT,
    empty_high_speed_rotation_score TEXT,
    idling_score TEXT,
    continuous_drive_score TEXT,
    wave_drive_score TEXT,
    safety_score TEXT,
    economy_score TEXT,
    total_score TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_kudgivt_organization_id ON kudgivt(organization_id);
CREATE INDEX idx_kudgivt_kudguri ON kudgivt(kudguri_uuid);

ALTER TABLE kudgivt ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON kudgivt
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));

-- kudgsir table (走行区間詳細)
CREATE TABLE kudgsir (
    uuid UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    organization_id UUID NOT NULL REFERENCES organizations(id),
    hash TEXT NOT NULL,
    kudguri_uuid UUID REFERENCES kudguri(uuid),
    unkou_no TEXT,
    read_date TEXT,
    office_cd TEXT,
    office_name TEXT,
    vehicle_cd TEXT,
    vehicle_name TEXT,
    driver_cd1 TEXT,
    driver_name1 TEXT,
    target_driver_type TEXT NOT NULL,
    target_driver_cd TEXT,
    target_driver_name TEXT,
    start_datetime TEXT,
    end_datetime TEXT,
    event_cd TEXT,
    event_name TEXT,
    start_mileage TEXT,
    end_mileage TEXT,
    section_time TEXT,
    section_distance TEXT,
    start_city_cd TEXT,
    start_city_name TEXT,
    end_city_cd TEXT,
    end_city_name TEXT,
    start_place_cd TEXT,
    start_place_name TEXT,
    end_place_cd TEXT,
    end_place_name TEXT,
    start_gps_valid TEXT,
    start_gps_lat TEXT,
    start_gps_lng TEXT,
    end_gps_valid TEXT,
    end_gps_lat TEXT,
    end_gps_lng TEXT,
    over_limit_max TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at TIMESTAMPTZ
);

CREATE INDEX idx_kudgsir_organization_id ON kudgsir(organization_id);
CREATE INDEX idx_kudgsir_kudguri ON kudgsir(kudguri_uuid);

ALTER TABLE kudgsir ENABLE ROW LEVEL SECURITY;

CREATE POLICY organization_isolation_policy ON kudgsir
    USING (organization_id::text = current_setting('app.current_organization_id', true))
    WITH CHECK (organization_id::text = current_setting('app.current_organization_id', true));
