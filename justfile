# `just check` is the definition of done: the gate workflow runs exactly it.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the recipes.
default:
    @just --list

# Every check, in order; the first failure stops the run.
check: fmt-check lint test

# rustfmt over every crate.
fmt-check:
    cargo fmt --all --check

# clippy over every target; warnings are errors.
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Unit and integration tests.
test:
    cargo test --workspace

# The release binary, in target/release/.
build:
    cargo build --release
