## Agent Behavior

- For any substantial feature implementation, refactor, fix, or chore, keep a log book under `.github/logs/*` with files named `dd_mm_yyyy.md`.

# Project Guidelines

** This is a living document, not a static spec. It should evolve as the project evolves, and it should be updated whenever the project shape, scope, or workflow changes. **

## Scope

- This repository is a Rust research workspace for a QUIC congestion-control augment built on Quinn.
- Favor reproducible experiments and clear methodology over broad feature work.
- Treat the codebase as a benchmarkable systems project, not a generic app scaffold.

## Architecture

- The repository root is a Cargo workspace with four crates under `crates/`.
- `amc-core` owns application-semantics and transport-policy logic.
- `demo-client` and `demo-server` own experiment traffic generation and sink behavior.
- `harness` owns scenario definitions, run orchestration, metrics export, and result packaging.

## Quinn usage

- Start from the public Quinn API and only propose a Quinn fork if the required congestion-control hooks are not exposed through `quinn::congestion` or `TransportConfig`.
- Keep congestion-control changes isolated behind explicit interfaces so baseline controllers and AMC runs can share the same harness.
- Preserve comparability: baseline controllers and AMC should run under the same workload and network scenario definitions.
- The core contribution is a semantic-aware transport policy with a congestion-control core, not a claim of building a BBRv2 replacement.

## Experiment design

- Optimize for repeatability and traceability.
- Prefer trace-driven or synthetic multimedia workloads over full media stacks unless realism clearly improves the evaluation.
- Use Linux `tc netem` as the preferred path-shaping mechanism for reproducible RTT, loss, and bandwidth control. Do not model full network topologies unless topology itself becomes part of the research question.
- Keep one semantic traffic class per connection unless mixed-traffic behavior is the specific subject under test.
- Every benchmark change should state which metric, scenario, or hypothesis it affects.
- Use streams for both VOD and live traffic in the main claim. Keep QUIC datagrams out of the primary evaluation unless they are a separate experimental axis.
- The application-to-transport interface should expose codec-agnostic semantic inputs such as deadline, importance, dependency depth, and freshness window.
- Do not assume raw codec labels such as GOP or frame type are directly meaningful to the congestion-control core without being translated into sender-visible utility signals.
- Prefer an offline `ffmpeg` and `ffprobe` preprocessing pipeline that converts open media assets into replayable segment sets and semantic traces rather than parsing full media stacks in the runtime sender.
- Keep large source media under `data/raw/` and out of Git. Small derived manifests or traces may be versioned only if they are lightweight and necessary for reproducibility.

## Measurement and reporting

- Separate raw result capture from processed summaries and figures.
- Keep scenario definitions in config or data files rather than hardcoding them into benchmark logic.
- When adding metrics, document their meaning and whether they are transport-level or application-level.
- Avoid changing benchmark methodology and algorithm logic in the same patch unless the coupling is unavoidable.
- Primary baseline comparisons should be Quinn NewReno, Cubic, and BBR.
- Multimedia gains must be reported together with fairness and coexistence results.

## Build and test

- Prefer `cargo check`, `cargo test`, and targeted crate-level commands before larger runs.
- Keep dependencies minimal and explicit at the workspace level.
- Do not introduce a Quinn fork, vendor tree, or custom patch dependency unless the project has already proven the public API is insufficient.

## Documentation

- Update `README.md` when the repo shape, build flow, or benchmark entry points change.
- Keep methodology and design notes in `docs/` and treat them as part of the research artifact.
- Write short, concrete docs that make the experiment path obvious to a future reviewer.
