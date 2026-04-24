# quinn-amc

`quinn-amc` is a Rust workspace for prototyping and evaluating an application-aware multimedia congestion-control augment on top of QUIC using Quinn.

The immediate goal is to build a reproducible research codebase that can:

- express application semantics for multimedia workloads such as VOD and live delivery
- map those semantics onto a semantic-aware transport policy with a congestion-control core
- benchmark the custom approach against baseline controllers such as NewReno, Cubic, and BBR
- generate figures and evidence suitable for a short conference-paper-style report

## Core idea

The project does not claim to be a BBRv2 alternative.

The working contribution is a semantic-aware transport policy for user-space QUIC. The sender can use application-level semantics to influence transport decisions before congestion pressure turns all queued bytes into equally important work.

For this project, the important sender-visible signals are codec-agnostic primitives such as:

- deadline
- importance
- dependency depth
- freshness window

The primary implementation path uses QUIC streams for both VOD and live traffic. QUIC datagrams are intentionally out of the main claim and can be studied later only as a secondary experiment axis.

## Workspace layout

```text
crates/
  amc-core/      # congestion-control and application-semantic core logic
  demo-client/   # sender / traffic generator / experiment client
  demo-server/   # receiver / sink / experiment server
  harness/       # scenario orchestration, metrics collection, result export

docs/
  core-idea.md   # thesis, scope, and contribution framing
  design.md      # semantic interface and transport-policy design
  evaluation.md  # benchmark questions, metrics, and scenario plan
  methodology.md # consolidated experiment and reporting plan

.github/
  copilot-instructions.md
```

## Current dependency stance

The project uses Quinn as a dependency, not a fork.

That is the correct starting point for this deadline because Quinn exposes custom congestion-control extension points through `quinn::congestion::{Controller, ControllerFactory}` and `TransportConfig::congestion_controller_factory(...)`.

Fork Quinn only if the public controller interface proves insufficient for the signals or hooks needed by the AMC design.

## Recommended toolchain workflow

Update Rust with `rustup`, which also updates `cargo` and `rustc` for the selected toolchain:

```powershell
rustup self update
rustup update stable
rustup component add rustfmt clippy
```

This repository pins Rust with `rust-toolchain.toml` for reproducible builds.

## Quinn feature selection

The workspace pins Quinn with this feature set:

- `runtime-tokio`: aligns with the async runtime we are likely to use for demos and orchestration
- `rustls-ring` and `ring`: stable TLS/crypto path for local experiments
- `platform-verifier`: useful when clients need platform certificate verification
- `log`: keeps protocol logging available during early bring-up

Not enabled by default here:

- `bloom`: acceptable but unnecessary for the current milestone
- `qlog`: useful later if packet-level trace export becomes part of the evaluation

## Methodology

See the project notes under `docs/`:

- [docs/core-idea.md](docs/core-idea.md) for the thesis and scope boundaries
- [docs/design.md](docs/design.md) for the application-to-transport semantic interface
- [docs/evaluation.md](docs/evaluation.md) for benchmark questions and metrics
- [docs/methodology.md](docs/methodology.md) for the consolidated experiment plan

## Near-term implementation plan

1. Extend the working Quinn demo client and server path into repeatable traffic-generation flows.
2. Define an application-to-transport semantic interface for `vod` and `live` traffic classes.
3. Implement baseline runs with Quinn-provided congestion controllers.
4. Add the AMC policy and congestion-control core under the same scenario matrix.
5. Export processed results and figures for the final report.

## Demo run

The current demo binaries establish a Quinn connection over QUIC, exchange one bidirectional stream message, and use a self-signed certificate that the client explicitly trusts.

Start the server in one terminal:

```powershell
cargo run -p demo-server -- --bind 127.0.0.1:5001 --cert-out demo-cert.der
```

Then run the client in a second terminal:

```powershell
cargo run -p demo-client -- --server 127.0.0.1:5001 --cert demo-cert.der --message "probe from demo-client"
```

Expected behavior:

- the server writes `demo-cert.der`, accepts one connection, reads one request, and replies with `echo:<message>`
- the client connects using `localhost` as the certificate name, sends the configured message, and logs the echoed response

## Build status

The workspace structure is bootstrapped, the demo client/server handshake path is working, and the AMC controller work remains to be built on top of that baseline.