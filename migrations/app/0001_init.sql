CREATE SCHEMA IF NOT EXISTS app;

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'data_service') THEN
        CREATE ROLE data_service;
    END IF;
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'storage_service') THEN
        CREATE ROLE storage_service;
    END IF;
END
$$;

GRANT USAGE ON SCHEMA app TO data_service, storage_service;

-- placeholder table; replaced by real tables in task 3.x
CREATE TABLE app.documents (
    id         UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    owner      TEXT        NOT NULL,
    title      TEXT        NOT NULL,
    body       TEXT        NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

GRANT SELECT, INSERT, UPDATE, DELETE ON app.documents TO data_service;
