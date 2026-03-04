-- Migration: SECURITY DEFINER functions for auth flows
--
-- Problem: migration 00033 enabled RLS on app_users, oauth_accounts,
-- user_organizations, organizations. Auth flows (login, register) run
-- BEFORE JWT is issued, so there is no org context -> all queries return 0 rows.
--
-- Solution: Wrap pre-auth queries in SECURITY DEFINER functions (same pattern
-- as resolve_sso_config / lookup_sso_config_for_login in migration 00032).

-- 1) Google OAuth user lookup
CREATE OR REPLACE FUNCTION find_google_user(p_provider_account_id TEXT)
RETURNS TABLE(user_id TEXT, org_id TEXT, email TEXT, org_slug TEXT)
LANGUAGE sql SECURITY DEFINER SET search_path = public
AS $$
    SELECT u.id::text, uo.organization_id::text, u.email, o.slug
    FROM oauth_accounts oa
    JOIN app_users u ON u.id = oa.app_user_id
    JOIN user_organizations uo ON uo.user_id = u.id AND uo.is_default = true
    JOIN organizations o ON o.id = uo.organization_id
    WHERE oa.provider = 'google' AND oa.provider_account_id = p_provider_account_id
      AND u.deleted_at IS NULL;
$$;

-- 2) Generic OAuth user lookup (SSO providers)
CREATE OR REPLACE FUNCTION find_oauth_user(p_provider TEXT, p_provider_account_id TEXT)
RETURNS TABLE(user_id TEXT, email TEXT)
LANGUAGE sql SECURITY DEFINER SET search_path = public
AS $$
    SELECT u.id::text, u.email
    FROM oauth_accounts oa
    JOIN app_users u ON u.id = oa.app_user_id
    WHERE oa.provider = p_provider AND oa.provider_account_id = p_provider_account_id
      AND u.deleted_at IS NULL;
$$;

-- 3) Auto-register new user (app_users + oauth_accounts + user_organizations)
CREATE OR REPLACE FUNCTION auto_register_user(
    p_email TEXT,
    p_display_name TEXT,
    p_avatar_url TEXT,
    p_provider TEXT,
    p_provider_account_id TEXT,
    p_access_token TEXT,
    p_organization_id UUID,
    p_role TEXT DEFAULT 'member'
) RETURNS TABLE(user_id TEXT, org_slug TEXT)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public
AS $$
DECLARE
    v_user_id UUID;
    v_org_slug TEXT;
BEGIN
    INSERT INTO app_users (email, display_name, avatar_url)
    VALUES (p_email, p_display_name, p_avatar_url)
    RETURNING id INTO v_user_id;

    INSERT INTO oauth_accounts (app_user_id, provider, provider_account_id, access_token)
    VALUES (v_user_id, p_provider, p_provider_account_id, p_access_token);

    INSERT INTO user_organizations (user_id, organization_id, role, is_default)
    VALUES (v_user_id, p_organization_id, p_role, true);

    SELECT slug INTO v_org_slug FROM organizations WHERE id = p_organization_id;

    RETURN QUERY SELECT v_user_id::text, v_org_slug;
END;
$$;

-- 4) Ensure org membership (for SSO re-login to different org)
CREATE OR REPLACE FUNCTION ensure_org_membership(p_user_id UUID, p_organization_id UUID)
RETURNS BOOLEAN
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public
AS $$
DECLARE
    v_exists BOOLEAN;
BEGIN
    SELECT EXISTS(
        SELECT 1 FROM user_organizations
        WHERE user_id = p_user_id AND organization_id = p_organization_id
    ) INTO v_exists;

    IF NOT v_exists THEN
        INSERT INTO user_organizations (user_id, organization_id, role, is_default)
        VALUES (p_user_id, p_organization_id, 'member', false);
    END IF;

    RETURN v_exists;
END;
$$;

-- 5) Password login user lookup
CREATE OR REPLACE FUNCTION find_password_user(p_org_id UUID, p_username TEXT)
RETURNS TABLE(app_user_id TEXT, password_hash TEXT, email TEXT, org_slug TEXT)
LANGUAGE sql SECURITY DEFINER SET search_path = public
AS $$
    SELECT pc.app_user_id::text, pc.password_hash, u.email, o.slug
    FROM password_credentials pc
    JOIN app_users u ON u.id = pc.app_user_id
    JOIN organizations o ON o.id = pc.organization_id
    WHERE pc.organization_id = p_org_id
      AND pc.username = p_username
      AND pc.enabled = true
      AND u.deleted_at IS NULL;
$$;

-- 6) Switch organization: verify membership + get user info
CREATE OR REPLACE FUNCTION get_user_org_for_switch(p_user_id UUID, p_organization_id UUID)
RETURNS TABLE(username TEXT, org_slug TEXT, role TEXT)
LANGUAGE sql SECURITY DEFINER SET search_path = public
AS $$
    SELECT
        COALESCE(au.email, au.display_name) as username,
        o.slug as org_slug,
        uo.role
    FROM app_users au
    JOIN user_organizations uo ON uo.user_id = au.id AND uo.organization_id = p_organization_id
    JOIN organizations o ON o.id = uo.organization_id
    WHERE au.id = p_user_id
      AND o.deleted_at IS NULL;
$$;

-- 7) List user's organizations
CREATE OR REPLACE FUNCTION list_user_orgs(p_user_id UUID)
RETURNS TABLE(org_id TEXT, org_name TEXT, org_slug TEXT, role TEXT, created_at TIMESTAMPTZ)
LANGUAGE sql SECURITY DEFINER SET search_path = public
AS $$
    SELECT o.id::text, o.name, o.slug, uo.role, o.created_at
    FROM organizations o
    JOIN user_organizations uo ON uo.organization_id = o.id
    WHERE uo.user_id = p_user_id
      AND o.deleted_at IS NULL
    ORDER BY o.created_at;
$$;

-- 8) Get org slug by id
CREATE OR REPLACE FUNCTION get_org_slug(p_org_id UUID)
RETURNS TEXT
LANGUAGE sql SECURITY DEFINER SET search_path = public
AS $$
    SELECT slug FROM organizations WHERE id = p_org_id;
$$;

-- 9) Sign up: create user + org + oauth + membership (for sign_up_with_google)
CREATE OR REPLACE FUNCTION signup_create_user_and_org(
    p_email TEXT,
    p_display_name TEXT,
    p_avatar_url TEXT,
    p_provider TEXT,
    p_provider_account_id TEXT,
    p_org_name TEXT,
    p_org_slug TEXT
) RETURNS TABLE(user_id TEXT, org_id TEXT)
LANGUAGE plpgsql SECURITY DEFINER SET search_path = public
AS $$
DECLARE
    v_user_id UUID;
    v_org_id UUID;
BEGIN
    INSERT INTO app_users (email, display_name, avatar_url)
    VALUES (p_email, p_display_name, p_avatar_url)
    RETURNING id INTO v_user_id;

    INSERT INTO oauth_accounts (app_user_id, provider, provider_account_id)
    VALUES (v_user_id, p_provider, p_provider_account_id);

    INSERT INTO organizations (name, slug)
    VALUES (p_org_name, p_org_slug)
    RETURNING id INTO v_org_id;

    INSERT INTO user_organizations (user_id, organization_id, role, is_default)
    VALUES (v_user_id, v_org_id, 'admin', true);

    RETURN QUERY SELECT v_user_id::text, v_org_id::text;
END;
$$;
