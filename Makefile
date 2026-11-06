.PHONY: build test up down bench lint

build:
	cargo build --release

up:
	docker compose up -d

down:
	docker compose down

test:
	cargo nextest run

lint:
	cargo fmt --check && cargo clippy --all-targets -- -D warnings

bench:
	echo "bench"
