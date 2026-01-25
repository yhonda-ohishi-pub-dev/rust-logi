-- Update dvr_notifications RLS policy to use UUID-based helper function
-- This aligns with the pattern established in migration 00013

-- Drop the old policy with TEXT comparison
DROP POLICY IF EXISTS organization_isolation_policy ON dvr_notifications;

-- Create new policy using UUID-based helper function (consistent with other tables)
CREATE POLICY organization_isolation_policy ON dvr_notifications
    USING (organization_id = get_current_organization_uuid())
    WITH CHECK (organization_id = get_current_organization_uuid());
