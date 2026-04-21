CREATE SCHEMA IF NOT EXISTS auth;

DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'auth_service') THEN
        CREATE ROLE auth_service;
    END IF;
END
$$;

GRANT USAGE ON SCHEMA auth TO auth_service;

-- users table (task 1.6)
CREATE TABLE auth.users (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT        NOT NULL UNIQUE,
    password_hash TEXT        NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

GRANT SELECT, INSERT, UPDATE, DELETE ON auth.users TO auth_service;
