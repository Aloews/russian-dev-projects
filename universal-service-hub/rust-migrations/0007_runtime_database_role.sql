-- Separate the application's runtime identity from the privileged migration connection.
-- The login used by USH must be able to SET ROLE to this non-login role.
DO $role$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'ush_runtime') THEN
        CREATE ROLE ush_runtime NOLOGIN NOSUPERUSER NOBYPASSRLS;
    END IF;
END;
$role$;

ALTER ROLE ush_runtime NOLOGIN NOSUPERUSER NOBYPASSRLS;
GRANT ush_runtime TO CURRENT_USER;
GRANT USAGE ON SCHEMA public TO ush_runtime;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO ush_runtime;
GRANT USAGE, SELECT, UPDATE ON ALL SEQUENCES IN SCHEMA public TO ush_runtime;

ALTER DEFAULT PRIVILEGES FOR ROLE CURRENT_USER IN SCHEMA public
    GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO ush_runtime;
ALTER DEFAULT PRIVILEGES FOR ROLE CURRENT_USER IN SCHEMA public
    GRANT USAGE, SELECT, UPDATE ON SEQUENCES TO ush_runtime;
