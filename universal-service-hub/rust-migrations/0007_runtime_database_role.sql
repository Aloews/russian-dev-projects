-- The privileged db-init step creates ush_runtime and grants SET ROLE to ush_app.
-- This migration runs as ush_app after db-init and grants the non-login runtime
-- identity only the schema/table/sequence privileges required by the API.
DO $role$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_roles
        WHERE rolname = 'ush_runtime'
          AND NOT rolsuper
          AND NOT rolbypassrls
          AND NOT rolcanlogin
    ) THEN
        RAISE EXCEPTION 'ush_runtime must exist as a non-login, non-superuser, NOBYPASSRLS role';
    END IF;
END;
$role$;

GRANT USAGE ON SCHEMA public TO ush_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO ush_runtime;
GRANT USAGE, SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA public TO ush_runtime;

ALTER DEFAULT PRIVILEGES FOR ROLE CURRENT_USER IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO ush_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE CURRENT_USER IN SCHEMA public
    GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO ush_runtime;
