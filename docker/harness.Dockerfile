# syntax=docker/dockerfile:1.7
FROM rust:1.95-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY configs ./configs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/workspace/target \
    cargo build --release -p harness && \
    cp /workspace/target/release/harness /tmp/harness

FROM gcr.io/distroless/cc-debian12

WORKDIR /workspace

COPY --from=builder /tmp/harness /usr/local/bin/harness
COPY --from=builder /workspace/configs ./configs

ENTRYPOINT ["/usr/local/bin/harness"]