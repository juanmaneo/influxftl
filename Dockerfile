# influxftl — reproducible OCI build.
#
# Plain OCI Dockerfile (no BuildKit-only syntax, fully-qualified image names) so that it
# builds identically with Docker, Podman and Buildah:
#
#   podman build -t influxftl .                  # runtime image: the default target
#   podman build --target dev -t influxftl-dev . # toolchain to run `cargo xtask dev-check`
#   docker build -t influxftl .
#   buildah bud -t influxftl .
#
# Stages, in build order. `podman build` and `buildah bud` build the LAST stage by default, so
# `runtime` must stay last; `dev` is only built when asked for with `--target dev`.
#   builder — compiles the release binary (internal, not a published target).
#   dev     — toolchain plus cargo-deny/audit/llvm-cov/kani (needs network while it builds).
#   runtime — minimal Debian image with the `influxftl` binary.
#
# Build arguments (keep RUST_VERSION in sync with rust-toolchain.toml):
#   --build-arg RUST_VERSION=1.99.1
#   --build-arg DEBIAN_VERSION=bookworm

ARG RUST_VERSION=1.99.1
ARG DEBIAN_VERSION=bookworm

# ---------------------------------------------------------------------------
# Builder: compile the release binary.
# ---------------------------------------------------------------------------
FROM docker.io/library/rust:${RUST_VERSION}-${DEBIAN_VERSION} AS builder

WORKDIR /src

# Copy the manifests and sources. `.dockerignore` keeps target/, reference/ and
# coverage artifacts out of the build context.
COPY . .

# Build only the binary crate (and its dependency, the warpscript library).
RUN cargo build --release --locked --package influxftl

# ---------------------------------------------------------------------------
# Dev: everything needed to run the full `cargo xtask dev-check` on any host.
# Build with:  podman build --target dev -t influxftl-dev .
# Run with:    podman run --rm influxftl-dev
# ---------------------------------------------------------------------------
FROM docker.io/library/rust:${RUST_VERSION}-${DEBIAN_VERSION} AS dev

# Kani needs Python 3; the coverage tooling needs the llvm-tools component.
RUN apt-get update \
    && apt-get install --yes --no-install-recommends python3 \
    && rm -rf /var/lib/apt/lists/*

RUN rustup component add rustfmt clippy llvm-tools-preview

# Pin the tool versions so the image is reproducible.
RUN cargo install --locked cargo-deny --version 0.20.2 \
    && cargo install --locked cargo-audit --version 0.22.2 \
    && cargo install --locked cargo-llvm-cov --version 0.8.7 \
    && cargo install --locked kani-verifier --version 0.68.0 \
    && cargo kani setup

WORKDIR /src
COPY . .

# Default command runs the complete pre-push chain.
CMD ["cargo", "xtask", "dev-check"]

# ---------------------------------------------------------------------------
# Runtime: minimal image with just the binary. Last stage, so it is the default target.
# ---------------------------------------------------------------------------
FROM docker.io/library/debian:${DEBIAN_VERSION}-slim AS runtime

# A non-root user to run the service.
RUN useradd --create-home --uid 10001 influxftl

COPY --from=builder /src/target/release/influxftl /usr/local/bin/influxftl

USER influxftl
WORKDIR /home/influxftl

ENTRYPOINT ["/usr/local/bin/influxftl"]
