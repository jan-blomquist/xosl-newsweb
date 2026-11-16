# syntax=docker/dockerfile:1
# The release image: the binary and nothing else, on a base without a shell or
# a package manager. The dev image is dev.Dockerfile.

# The Rust version is the one in rust-toolchain.toml: bump the two together.
# rust-toolchain.toml itself stays out of the build context, so rustup
# downloads no component the build does not use.
FROM rust:1.100.0-slim-trixie@sha256:e9d95b74296f7126a83ae3d041aeaba1ff4c478f13abc5cc034fcf7c131c3d4a AS build
# The C toolchain with cmake: SQLite and the TLS library are compiled into the
# binary.
RUN apt-get update \
 && apt-get install -y --no-install-recommends build-essential cmake pkg-config \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
# --locked: the build fails when Cargo.lock does not match Cargo.toml. The
# empty /data is copied below, so that the data folder belongs to the user the
# service runs as.
RUN cargo build --release --locked \
 && cp target/release/xosl-newsweb /xosl-newsweb \
 && mkdir /data

# glibc, libgcc and the CA certificates; runs as user 65532, not root.
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2
COPY --from=build /xosl-newsweb /xosl-newsweb
COPY --from=build --chown=65532:65532 /data /data

# Inside a container the server listens on every interface; which host
# address reaches it is decided where the port is published.
ENV XOSL_NEWSWEB_DATA_DIR=/data \
    XOSL_NEWSWEB_LISTEN=0.0.0.0:50051
VOLUME /data
EXPOSE 50051

# The binary asks its own server; the image has no shell and no curl.
HEALTHCHECK --interval=30s --timeout=10s --start-period=60s --retries=3 \
    CMD ["/xosl-newsweb", "health"]

ENTRYPOINT ["/xosl-newsweb"]
CMD ["run"]
