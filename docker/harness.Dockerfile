FROM rust:1.95-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY configs ./configs

RUN cargo build --release -p harness

FROM gcr.io/distroless/cc-debian12

WORKDIR /workspace

COPY --from=builder /workspace/target/release/harness /usr/local/bin/harness
COPY --from=builder /workspace/configs ./configs

ENTRYPOINT ["/usr/local/bin/harness"]