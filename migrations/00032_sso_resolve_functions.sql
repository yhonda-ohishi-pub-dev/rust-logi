-- SECURITY DEFINER functions for SSO config resolution
-- These functions bypass RLS because SSO resolution happens BEFORE authentication
-- (we don't know the organization_id yet - that's what we're resolving)

-- Function 1: For ResolveSsoProvider RPC (OAuth redirect - pre-auth lookup)
-- Returns only public-safe fields (no client_secret)
CREATE OR REPLACE FUNCTION resolve_sso_config(p_provider TEXT, p_external_org_id TEXT)
RETURNS TABLE(client_id TEXT, organization_name TEXT, woff_id TEXT)
LANGUAGE plpgsql SECURITY DEFINER AS $$
BEGIN
    -- Primary: exact match on external_org_id
    RETURN QUERY
    SELECT c.client_id, o.name, c.woff_id
    FROM sso_provider_configs c
    JOIN organizations o ON o.id = c.organization_id
    WHERE c.provider = p_provider
      AND c.external_org_id = p_external_org_id
      AND c.enabled = TRUE
      AND o.deleted_at IS NULL
    LIMIT 1;

    IF NOT FOUND THEN
        -- Fallback: org slug match
        RETURN QUERY
        SELECT c.client_id, o.name, c.woff_id
        FROM organizations o
        JOIN sso_provider_configs c ON c.organization_id = o.id
        WHERE o.slug = p_external_org_id
          AND c.provider = p_provider
          AND c.enabled = TRUE
          AND o.deleted_at IS NULL
        LIMIT 1;
    END IF;
END;
$$;

-- Function 2: For LoginWithSsoProvider RPC (OAuth callback - auth flow)
-- Returns client_secret for code exchange
CREATE OR REPLACE FUNCTION lookup_sso_config_for_login(p_provider TEXT, p_external_org_id TEXT)
RETURNS TABLE(client_id TEXT, client_secret_encrypted TEXT, organization_id TEXT, org_slug TEXT)
LANGUAGE plpgsql SECURITY DEFINER AS $$
BEGIN
    RETURN QUERY
    SELECT c.client_id, c.client_secret_encrypted, c.organization_id::text, o.slug
    FROM sso_provider_configs c
    JOIN organizations o ON o.id = c.organization_id
    WHERE c.provider = p_provider
      AND c.external_org_id = p_external_org_id
      AND c.enabled = TRUE
    LIMIT 1;
END;
$$;

-- Grant execute to application user
GRANT EXECUTE ON FUNCTION resolve_sso_config(TEXT, TEXT) TO rust_logi_app;
GRANT EXECUTE ON FUNCTION lookup_sso_config_for_login(TEXT, TEXT) TO rust_logi_app;
