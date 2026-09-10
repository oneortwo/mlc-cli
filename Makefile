.PHONY: build test lint check release install
build:
	cargo build --locked
test:
	cargo test --locked
lint:
	cargo fmt --check
	cargo clippy --locked --all-targets -- -D warnings
check: lint test
release:
	cargo build --release --locked
install:
	cargo install --path . --locked
