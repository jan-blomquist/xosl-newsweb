# `just check` is the definition of done: the gate workflow runs exactly it.

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the recipes.
default:
    @just --list

# Every check, in order; the first failure stops the run.
check: fmt-check lint buf-lint test test-integration

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

# The unit tests, inline in src/: in memory, no file, database or network.
test:
    cargo test --lib --bins

# The integration tests under tests/, except the live ones: they run SQLite.
test-integration:
    cargo test --test '*'

# The tests that call the real exchange: a few requests, one per second.
test-live:
    cargo test --test live -- --ignored --nocapture

# The release binary, in target/release/.
build:
    cargo build --release
