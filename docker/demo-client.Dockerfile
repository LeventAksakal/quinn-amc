FROM rust:1.95-bookworm AS builder

WORKDIR /workspace

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates
COPY configs ./configs

RUN cargo build --release -p demo-client

FROM gcr.io/distroless/cc-debian12

WORKDIR /workspace

COPY --from=builder /workspace/target/release/demo-client /usr/local/bin/demo-client

ENTRYPOINT ["/usr/local/bin/demo-client"]