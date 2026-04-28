FROM rust:1.95-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY configs ./configs

RUN cargo build --release -p harness

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates iproute2 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

COPY --from=builder /workspace/target/release/harness /usr/local/bin/harness
COPY --from=builder /workspace/configs ./configs
RUN mkdir -p /workspace/data /workspace/results

ENTRYPOINT ["harness"]