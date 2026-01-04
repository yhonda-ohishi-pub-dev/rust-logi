-- Migration: Add index for listCarInspections ORDER BY and getCarInspection lookup

-- Index for listCarInspections ORDER BY GrantdateY, GrantdateM, GrantdateD
CREATE INDEX idx_car_inspection_grantdate ON car_inspection(
    organization_id,
    "GrantdateY" DESC,
    "GrantdateM" DESC,
    "GrantdateD" DESC
);
