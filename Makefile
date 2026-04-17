.PHONY: build lint test

all: build

build:
	cargo build --profile release

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
	cargo test --all-features

install:
	cargo install --path .
