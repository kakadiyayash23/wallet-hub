.PHONY: db api sports web test lint build

db:            ## start only Postgres in Docker
	docker compose up -d postgres

api:           ## run the Rust api with hot env from .env
	cargo run -p api

sports:        ## run the mock sportsbook
	cargo run -p mock-sportsbook

web:           ## run the Vue dev server
	cd web && npm run dev

test:          ## rust unit + integration tests (integration needs DATABASE_URL) and web tests
	cargo test --workspace
	cd web && npm test

lint:
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets -- -D warnings
	cd web && npm run typecheck

build:
	cargo build --release --workspace
	cd web && npm run build
