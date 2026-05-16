# syntax=docker/dockerfile:1.7
FROM rust:1.95-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY configs ./configs

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/workspace/target \
    cargo build --release -p demo-server && \
    cp /workspace/target/release/demo-server /tmp/demo-server

FROM gcr.io/distroless/cc-debian12

WORKDIR /workspace

COPY --from=builder /tmp/demo-server /usr/local/bin/demo-server

EXPOSE 5000/udp

ENTRYPOINT ["/usr/local/bin/demo-server"]