.PHONY: build lint test test-input publish

VERSION := 0.1.1

all: build

build:
	cargo build --profile release

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings

test: test-input
	cargo test --workspace --all-features

test-input:
	@mkdir -p target
	xcrun clang -Wall -Wextra -Werror -fsanitize=address \
		nanomiddleclick-input/tests/device_lifetime.c \
		-framework ApplicationServices -framework CoreFoundation -framework IOKit \
		-o target/device_lifetime_test
	./target/device_lifetime_test

install:
	cargo install --path nanomiddleclick

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
