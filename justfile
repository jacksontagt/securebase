set dotenv-load := true

default:
    @just --list

up:
    docker compose up -d
    @echo "postgres: localhost:5432   garage s3: localhost:3900   garage admin: localhost:3903"

down:
    docker compose down

destroy:
    docker compose down -v --remove-orphans

logs service="":
    docker compose logs -f {{service}}

migrate:
    sqlx migrate run --source migrations/acl --database-url "$DATABASE_URL"
    sqlx migrate run --source migrations/app --database-url "$DATABASE_URL"

setup: up migrate

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets -- -D warnings
