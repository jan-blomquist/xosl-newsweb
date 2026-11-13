# syntax=docker/dockerfile:1
# check=skip=InvalidDefaultArgInFrom
# The dev image: the toolchain, on the image vz passes as BASE (vz.yml). It is
# not the image the service ships in. Tools live under /usr/local, never in a
# home.
ARG BASE

# Vendor releases, each pinned by sha256; bind-mounted below, so no layer keeps them.
FROM scratch AS bins
ADD --checksum=sha256:4a5cc2f53e6f0f8c59092a6cc38291eb729d46a7dd95d3ae582008881b84931d \
    https://github.com/casey/just/releases/download/1.58.0/just-1.58.0-x86_64-unknown-linux-musl.tar.gz /just.tgz
# buf and its three plugins generate the Rust code of the gRPC contract.
ADD --checksum=sha256:2b1f9cfb5e17d50c10dea9202979ffd28ca7ff7a6f4e51e801a9463986690b03 \
    https://github.com/bufbuild/buf/releases/download/v1.69.0/buf-Linux-x86_64.tar.gz /buf.tgz
ADD --checksum=sha256:038a43ee2df38504bc28b9adc078733622be367af9f58dd874c7e4e02b2b8756 \
    https://github.com/cargo-bins/cargo-quickinstall/releases/download/protoc-gen-prost-0.5.0/protoc-gen-prost-0.5.0-x86_64-unknown-linux-musl.tar.gz /protoc-gen-prost.tgz
ADD --checksum=sha256:0eb4ce2f823371abfc82631b7e11ff8c93ca196453b9ec55256b51d2826decae \
    https://github.com/cargo-bins/cargo-quickinstall/releases/download/protoc-gen-prost-serde-0.4.0/protoc-gen-prost-serde-0.4.0-x86_64-unknown-linux-musl.tar.gz /protoc-gen-prost-serde.tgz
ADD --checksum=sha256:44dbd20acdb50454380918ab6de336dd1fc55e4f5e6788b8a886e598811fba4d \
    https://github.com/cargo-bins/cargo-quickinstall/releases/download/protoc-gen-tonic-0.5.0/protoc-gen-tonic-0.5.0-x86_64-unknown-linux-musl.tar.gz /protoc-gen-tonic.tgz
ADD --chmod=755 --checksum=sha256:dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71 \
    https://static.rust-lang.org/rustup/archive/1.29.1/x86_64-unknown-linux-gnu/rustup-init /rustup-init

FROM ${BASE}
ARG DEBIAN_FRONTEND=noninteractive
# The C toolchain (SQLite is compiled into the binary) and mold for linking,
# the sqlite3 shell for looking into the database, the shell tools.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential curl fish git jq less mold nano pkg-config procps \
      ripgrep sqlite3 tmux tree \
 && rm -rf /var/lib/apt/lists/*

RUN --mount=type=bind,from=bins,target=/tmp/bins \
    set -e; cd /tmp/bins; bin=/usr/local/bin; \
    tar -xzf just.tgz -C $bin --no-same-owner just; \
    tar -xzf buf.tgz -C /usr/local --strip-components=1 --no-same-owner buf/bin buf/share; \
    for p in protoc-gen-prost protoc-gen-prost-serde protoc-gen-tonic; do \
      tar -xzf $p.tgz -C $bin --no-same-owner $p; \
    done

# Rust where the official image keeps it, writable by every user. The version
# is the one in rust-toolchain.toml: bump the two together. Crate downloads are
# state in vz.yml.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN --mount=type=bind,from=bins,source=/rustup-init,target=/tmp/rustup-init \
    /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain 1.99.1 \
      -c rustfmt -c clippy -c rust-src -c rust-analyzer \
 && chmod -R a+w "$RUSTUP_HOME" "$CARGO_HOME"

# Dev-profile debuginfo trimmed to line tables; mold links.
ENV CARGO_PROFILE_DEV_DEBUG=line-tables-only \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
    EDITOR=nano

# No greeting; completions for `just` in every user's fish.
RUN printf 'set -g fish_greeting\n' > /etc/fish/conf.d/vz.fish \
 && just --completions fish > /usr/share/fish/vendor_completions.d/just.fish
