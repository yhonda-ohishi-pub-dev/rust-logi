-- Migration: Add indexes for list_current_car_inspections query optimization

-- Index for car_inspection table: WHERE and ORDER BY on TwodimensionCodeInfoValidPeriodExpirdate
CREATE INDEX idx_car_inspection_valid_period ON car_inspection(
    organization_id,
    "TwodimensionCodeInfoValidPeriodExpirdate"
);

-- Index for car_inspection_files_a subquery lookup
CREATE INDEX idx_car_inspection_files_a_lookup ON car_inspection_files_a(
    organization_id,
    "ElectCertMgNo",
    "GrantdateE",
    "GrantdateY",
    "GrantdateM",
    "GrantdateD",
    type,
    deleted_at
);

-- Index for car_inspection_files_b subquery lookup
CREATE INDEX idx_car_inspection_files_b_lookup ON car_inspection_files_b(
    organization_id,
    "ElectCertMgNo",
    "GrantdateE",
    "GrantdateY",
    "GrantdateM",
    "GrantdateD",
    type,
    deleted_at
);
