CREATE SCHEMA IF NOT EXISTS acl;

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'acl_service') THEN
        CREATE ROLE acl_service;
    END IF;
END
$$;

GRANT USAGE ON SCHEMA acl TO acl_service;

CREATE TABLE IF NOT EXISTS acl.schemas (
    id      SERIAL PRIMARY KEY,
    name    TEXT NOT NULL,
    version INT NOT NULL,
    body    TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (name, version)
);

GRANT SELECT, INSERT, UPDATE, DELETE ON acl.schemas TO acl_service;
GRANT USAGE, SELECT ON SEQUENCE acl.schemas_id_seq TO acl_service;
