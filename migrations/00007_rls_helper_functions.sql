-- Migration: Create RLS helper functions and application roles

-- Create application role for normal users (subject to RLS)
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'app_user') THEN
        CREATE ROLE app_user NOLOGIN;
    END IF;
END
$$;

-- Create admin role (bypasses RLS)
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'app_admin') THEN
        CREATE ROLE app_admin NOLOGIN;
    END IF;
END
$$;

-- Grant permissions to app_user
GRANT USAGE ON SCHEMA public TO app_user;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO app_user;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO app_user;

-- Grant bypass permissions to app_admin
GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO app_admin;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO app_admin;
ALTER ROLE app_admin BYPASSRLS;

-- Helper function to set current organization
CREATE OR REPLACE FUNCTION set_current_organization(org_id TEXT)
RETURNS VOID AS $$
BEGIN
    PERFORM set_config('app.current_organization_id', org_id, false);
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

-- Helper function to get current organization
CREATE OR REPLACE FUNCTION get_current_organization()
RETURNS TEXT AS $$
BEGIN
    RETURN current_setting('app.current_organization_id', true);
END;
$$ LANGUAGE plpgsql STABLE;

-- Helper function to validate organization exists
CREATE OR REPLACE FUNCTION validate_organization(org_id UUID)
RETURNS BOOLEAN AS $$
DECLARE
    org_exists BOOLEAN;
BEGIN
    SELECT EXISTS(
        SELECT 1 FROM organizations
        WHERE id = org_id
        AND deleted_at IS NULL
    ) INTO org_exists;
    RETURN org_exists;
END;
$$ LANGUAGE plpgsql STABLE SECURITY DEFINER;

-- Trigger function to auto-set organization_id on insert if not provided
CREATE OR REPLACE FUNCTION set_organization_id_on_insert()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.organization_id IS NULL THEN
        NEW.organization_id := current_setting('app.current_organization_id', true)::UUID;
    END IF;

    -- Validate the organization exists
    IF NOT validate_organization(NEW.organization_id) THEN
        RAISE EXCEPTION 'Invalid organization_id: %', NEW.organization_id;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Comment for documentation
COMMENT ON FUNCTION set_current_organization(TEXT) IS
'Sets the current organization for RLS policies. Call this at the beginning of each request/transaction.
Example: SELECT set_current_organization(''00000000-0000-0000-0000-000000000001'');';

COMMENT ON FUNCTION get_current_organization() IS
'Returns the current organization ID set in the session.';
