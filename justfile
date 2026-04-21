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

fmt:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets -- -D warnings
