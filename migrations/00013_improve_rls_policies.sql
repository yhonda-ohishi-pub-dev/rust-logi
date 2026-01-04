-- Migration: Improve RLS policies to use UUID comparison instead of TEXT
-- This allows PostgreSQL to use indexes on organization_id column

-- Helper function to get current organization as UUID
CREATE OR REPLACE FUNCTION get_current_organization_uuid() RETURNS UUID AS $$
BEGIN
    RETURN current_setting('app.current_organization_id', true)::uuid;
EXCEPTION
    WHEN OTHERS THEN
        RETURN NULL;
END;
$$ LANGUAGE plpgsql STABLE;

-- car_inspection
DROP POLICY IF EXISTS organization_isolation_policy ON car_inspection;
CREATE POLICY organization_isolation_policy ON car_inspection
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- car_inspection_files
DROP POLICY IF EXISTS organization_isolation_policy ON car_inspection_files;
CREATE POLICY organization_isolation_policy ON car_inspection_files
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- car_inspection_files_a
DROP POLICY IF EXISTS organization_isolation_policy ON car_inspection_files_a;
CREATE POLICY organization_isolation_policy ON car_inspection_files_a
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- car_inspection_files_b
DROP POLICY IF EXISTS organization_isolation_policy ON car_inspection_files_b;
CREATE POLICY organization_isolation_policy ON car_inspection_files_b
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- car_inspection_deregistration
DROP POLICY IF EXISTS organization_isolation_policy ON car_inspection_deregistration;
CREATE POLICY organization_isolation_policy ON car_inspection_deregistration
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- car_inspection_deregistration_files
DROP POLICY IF EXISTS organization_isolation_policy ON car_inspection_deregistration_files;
CREATE POLICY organization_isolation_policy ON car_inspection_deregistration_files
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- files
DROP POLICY IF EXISTS organization_isolation_policy ON files;
CREATE POLICY organization_isolation_policy ON files
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- files_append
DROP POLICY IF EXISTS organization_isolation_policy ON files_append;
CREATE POLICY organization_isolation_policy ON files_append
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- ichiban_cars
DROP POLICY IF EXISTS organization_isolation_policy ON ichiban_cars;
CREATE POLICY organization_isolation_policy ON ichiban_cars
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- car_ins_sheet_ichiban_cars
DROP POLICY IF EXISTS organization_isolation_policy ON car_ins_sheet_ichiban_cars;
CREATE POLICY organization_isolation_policy ON car_ins_sheet_ichiban_cars
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- car_ins_sheet_ichiban_cars_a
DROP POLICY IF EXISTS organization_isolation_policy ON car_ins_sheet_ichiban_cars_a;
CREATE POLICY organization_isolation_policy ON car_ins_sheet_ichiban_cars_a
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- dtako_cars_ichiban_cars
DROP POLICY IF EXISTS organization_isolation_policy ON dtako_cars_ichiban_cars;
CREATE POLICY organization_isolation_policy ON dtako_cars_ichiban_cars
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- cam_files
DROP POLICY IF EXISTS organization_isolation_policy ON cam_files;
CREATE POLICY organization_isolation_policy ON cam_files
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- stuff
DROP POLICY IF EXISTS organization_isolation_policy ON stuff;
CREATE POLICY organization_isolation_policy ON stuff
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- kudguri
DROP POLICY IF EXISTS organization_isolation_policy ON kudguri;
CREATE POLICY organization_isolation_policy ON kudguri
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- kudgcst
DROP POLICY IF EXISTS organization_isolation_policy ON kudgcst;
CREATE POLICY organization_isolation_policy ON kudgcst
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- kudgfry
DROP POLICY IF EXISTS organization_isolation_policy ON kudgfry;
CREATE POLICY organization_isolation_policy ON kudgfry
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- kudgful
DROP POLICY IF EXISTS organization_isolation_policy ON kudgful;
CREATE POLICY organization_isolation_policy ON kudgful
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- kudgivt
DROP POLICY IF EXISTS organization_isolation_policy ON kudgivt;
CREATE POLICY organization_isolation_policy ON kudgivt
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- kudgsir
DROP POLICY IF EXISTS organization_isolation_policy ON kudgsir;
CREATE POLICY organization_isolation_policy ON kudgsir
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- uriage_jisha
DROP POLICY IF EXISTS organization_isolation_policy ON uriage_jisha;
CREATE POLICY organization_isolation_policy ON uriage_jisha
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());

-- data_archives (has multiple policies)
DROP POLICY IF EXISTS data_archives_select ON data_archives;
DROP POLICY IF EXISTS data_archives_update ON data_archives;
DROP POLICY IF EXISTS data_archives_delete ON data_archives;
CREATE POLICY data_archives_select ON data_archives FOR SELECT
    USING (organization_id = get_current_organization_uuid());
CREATE POLICY data_archives_update ON data_archives FOR UPDATE
    USING (organization_id = get_current_organization_uuid());
CREATE POLICY data_archives_delete ON data_archives FOR DELETE
    USING (organization_id = get_current_organization_uuid());

-- data_archive_logs (uses JOIN, update to use UUID)
DROP POLICY IF EXISTS data_archive_logs_select ON data_archive_logs;
CREATE POLICY data_archive_logs_select ON data_archive_logs FOR SELECT
    USING (EXISTS (
        SELECT 1 FROM data_archives da
        WHERE da.id = data_archive_logs.archive_id
          AND da.organization_id = get_current_organization_uuid()
    ));

-- file_access_logs (already uses get_current_organization, update to UUID version)
DROP POLICY IF EXISTS file_access_logs_org_isolation ON file_access_logs;
CREATE POLICY file_access_logs_org_isolation ON file_access_logs
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());
