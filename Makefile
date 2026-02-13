.PHONY: help build build-debug check clean clippy fmt fmt-check test run install release

help:
	@echo "Available targets:"
	@echo "  build        - Build release version (default)"
	@echo "  build-debug  - Build debug version"
	@echo "  release      - Alias for build"
	@echo "  check        - Run cargo check"
	@echo "  clean        - Clean build artifacts"
	@echo "  clippy       - Run clippy linter"
	@echo "  fmt          - Format code"
	@echo "  fmt-check    - Check code formatting"
	@echo "  test         - Run tests"
	@echo "  run          - Run the application"
	@echo "  install      - Install binary to ~/.cargo/bin"

build release: check
	cargo build --release --verbose

build-debug:
	cargo build --verbose

check:
	cargo check --all-targets

clean:
	cargo clean

clippy:
	cargo clippy --all-targets -- -D warnings

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

test:
	cargo test --all

run:
	cargo run

install:
	cargo install --path .
