# `just check` is the definition of done: the gate workflow runs exactly it.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the recipes.
default:
    @just --list

# Every check, in order; the first failure stops the run.
check: fmt-check lint buf-lint test

# rustfmt over the crate.
fmt-check:
    cargo fmt --check

# clippy over every target; warnings are errors.
lint:
    cargo clippy --all-targets -- -D warnings

# The contract passes buf's STANDARD rules, with no exceptions.
buf-lint:
    cd contracts && buf lint

# Regenerate src/gen from the .proto files; commit the result.
gen:
    cd contracts && buf generate --template newsweb/buf.gen.yaml

# Unit and integration tests.
test:
    cargo test

# The tests that call the real exchange: a few requests, one per second.
test-live:
    cargo test -- --ignored --nocapture live

# The release binary, in target/release/.
build:
    cargo build --release
