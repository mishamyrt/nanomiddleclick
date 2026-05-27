# Contributing

Thanks for your interest in contributing to `nanomiddleclick`.

This project is a lightweight macOS daemon written in Rust, with a small native shim for platform-specific input handling. Please keep changes focused and avoid adding dependencies unless they are clearly needed.

## Requirements

- macOS, for building and testing the full project.
- A recent Rust toolchain installed with `rustup`.
- `rustfmt` and `clippy` components installed.

You can install the required Rust components with:

```sh
rustup component add rustfmt clippy
```

## Project Layout

- `nanomiddleclick` contains the CLI and daemon application.
- `nanomiddleclick-core` contains configuration and gesture orchestration.
- `nanomiddleclick-input` contains macOS input runtime code and the native multitouch/event-tap shim.
- `nanomiddleclick-preferences` contains generic macOS preferences access.
- `nanomiddleclick-app-monitor` contains generic macOS workspace/frontmost-app monitoring.
- `.github/workflows/qa.yml` defines the checks that run in CI.

## Development Workflow

Build the project:

```sh
make
```

Run tests:

```sh
make test
```

Run lint checks:

```sh
make lint
```

Check formatting before submitting changes:

```sh
cargo fmt --all --check
```

If formatting is needed, run:

```sh
cargo fmt --all
```

You can install a local build with:

```sh
make install
```

## Testing Changes

Before opening a pull request, run:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

For changes that affect input handling, configuration, daemon startup, or launchd integration, also test the installed daemon manually on macOS. Include the scenario you tested in the pull request description.

## Code Style

- Keep changes small, direct, and easy to review.
- Prefer clear Rust code over clever abstractions.
- Keep product-specific orchestration in `nanomiddleclick` or `nanomiddleclick-core`.
- Keep reusable macOS behavior in the focused platform crates and their native shims.
- Avoid new dependencies unless they materially simplify the implementation or improve correctness.
- Do not commit generated build artifacts from `target/`.

The workspace enables strict Clippy checks. Treat warnings as issues to fix unless there is a strong reason to adjust the lint configuration.

## Documentation

Update `README.md` when a user-facing command, configuration option, installation step, or behavior changes.

Update `CHANGELOG.md` only as part of the release process; it is generated with `git-cliff`.

## Pull Requests

When opening a pull request:

- Explain what changed and why.
- Link related issues when applicable.
- List the checks you ran locally.
- Mention any manual macOS testing performed.
- Keep unrelated cleanup or refactoring out of feature and bug-fix PRs.

CI must pass before a pull request can be merged.

## Releases

Releases are handled by the maintainer. The repository uses `cargo-dist` for release artifacts and `git-cliff` for changelog generation.
