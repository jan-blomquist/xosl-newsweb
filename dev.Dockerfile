# syntax=docker/dockerfile:1
# check=skip=InvalidDefaultArgInFrom
# The dev image: the toolchain, on the image vz passes as BASE (vz.yml). The
# image the service ships in is the Dockerfile beside this one. Tools live
# under /usr/local, never in a home.
ARG BASE

# Vendor releases, each pinned by sha256; bind-mounted below, so no layer keeps them.
FROM scratch AS bins
ADD --checksum=sha256:409044617b6f06517c1035e55ca38c8c856ff8b650e41bc2aa1ef82d5903a23e \
    https://github.com/casey/just/releases/download/1.59.0/just-1.59.0-x86_64-unknown-linux-musl.tar.gz /just.tgz
# buf and its three plugins generate the Rust code of the gRPC contract.
ADD --checksum=sha256:7168a06e29bd5eb8d1530848f242264f7a86b9b8af3b2e811570fbea0c929a30 \
    https://github.com/bufbuild/buf/releases/download/v1.75.0/buf-Linux-x86_64.tar.gz /buf.tgz
ADD --checksum=sha256:038a43ee2df38504bc28b9adc078733622be367af9f58dd874c7e4e02b2b8756 \
    https://github.com/cargo-bins/cargo-quickinstall/releases/download/protoc-gen-prost-0.5.0/protoc-gen-prost-0.5.0-x86_64-unknown-linux-musl.tar.gz /protoc-gen-prost.tgz
ADD --checksum=sha256:0eb4ce2f823371abfc82631b7e11ff8c93ca196453b9ec55256b51d2826decae \
    https://github.com/cargo-bins/cargo-quickinstall/releases/download/protoc-gen-prost-serde-0.4.0/protoc-gen-prost-serde-0.4.0-x86_64-unknown-linux-musl.tar.gz /protoc-gen-prost-serde.tgz
ADD --checksum=sha256:44dbd20acdb50454380918ab6de336dd1fc55e4f5e6788b8a886e598811fba4d \
    https://github.com/cargo-bins/cargo-quickinstall/releases/download/protoc-gen-tonic-0.5.0/protoc-gen-tonic-0.5.0-x86_64-unknown-linux-musl.tar.gz /protoc-gen-tonic.tgz
ADD --chmod=755 --checksum=sha256:b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55 \
    https://static.rust-lang.org/rustup/archive/1.29.3/x86_64-unknown-linux-gnu/rustup-init /rustup-init

FROM ${BASE}
ARG DEBIAN_FRONTEND=noninteractive
# The C toolchain with cmake (SQLite and the TLS library are compiled into the
# binary) and mold for linking, the sqlite3 shell for looking into the
# database, the shell tools.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential cmake curl fish git jq less mold nano pkg-config procps \
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
    /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain 1.100.0 \
      -c rustfmt -c clippy -c rust-src -c rust-analyzer \
 && chmod -R a+w "$RUSTUP_HOME" "$CARGO_HOME"

# Dev-profile debuginfo trimmed to line tables; mold links.
ENV CARGO_PROFILE_DEV_DEBUG=line-tables-only \
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C link-arg=-fuse-ld=mold" \
    EDITOR=nano

# No greeting; completions for `just` in every user's fish.
RUN printf 'set -g fish_greeting\n' > /etc/fish/conf.d/vz.fish \
 && just --completions fish > /usr/share/fish/vendor_completions.d/just.fish
