.PHONY: build lint test publish

VERSION := 0.0.1

all: build

build:
	cargo build --profile release

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test:
	cargo test --all-features

install:
	cargo install --path .

publish:
	@sed -E 's/^version = "[^"]+"/version = "${VERSION}"/' Cargo.toml > Cargo.toml.tmp
	@mv Cargo.toml.tmp Cargo.toml
	@cargo update -p nanomiddleclick
	@git add Makefile Cargo.toml Cargo.lock
	@git commit -m "chore: release ${VERSION} 🔥"
	@git tag "v${VERSION}"
	@git-cliff -o CHANGELOG.md
	@git tag -d "v${VERSION}"
	@git add CHANGELOG.md
	@git commit --amend --no-edit
	@git tag -a "v${VERSION}" -m "release v${VERSION}"
	@git push
	@git push --tags
