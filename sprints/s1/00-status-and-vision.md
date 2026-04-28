# Sprint 1: Status and Vision

## Purpose

This document is the shared baseline for Sprint 1. Every parallel workstream should use it as the source of truth for current status, architectural constraints, and the integration target.

## Current Status

### What already exists

- A Cargo workspace with four crates: `amc-core`, `demo-client`, `demo-server`, and `harness`.
- A working Quinn-based client/server replay path that can send preprocessed media artifacts over QUIC.
- Configurable controller selection through Quinn public congestion-controller factories, including `new_reno`, `cubic`, `bbr`, and the current `amc_preview` path.
- Offline media preprocessing scripts that generate CMAF-style fragmented outputs and replay manifests.
- Replay manifests with semantic hints attached to segments and defaults attached to assets.
- A server-side raw reporting path that records segment arrival timing, lateness, usefulness, and optional AMC runtime telemetry.
- A harness with `run-suite` for local development runs and `analyze-suite` for offline analysis.
- Local processed outputs under `results/processed/harness/` and raw reports under `results/raw/`, including a documented Sprint 1 result schema note.
- A first live AMC preview controller path that consumes sender-computed utility signals and changes congestion-window growth and loss response through Quinn's public controller hooks.
- Dockerfiles and a compose topology aligned to a single-host Linux VPS experiment model.
- A host-side Linux experiment runner scaffold for container lifecycle and host-managed `tc` application.

### What is validated

- The demo client and server compile and run.
- The replay path can transfer full preprocessed assets.
- VOD and live replay modes exist.
- Raw transfer reports are generated.
- Processed harness summaries are generated.
- The same local workload has been captured under at least two baseline controllers on Windows loopback.
- A local `amc_preview` run has produced raw and processed artifacts with populated runtime utility telemetry.
- The Windows local `run-suite` path now completes cleanly after successful transfers and waits for a valid freshly written certificate instead of relying on a blind fixed sleep.
- The current architecture, methodology, and replay-semantics model are documented.

### What is not done yet

- Linux VPS orchestration has been designed and scaffolded, but it still needs end-to-end validation on Linux.
- The current AMC path is still a constrained preview that uses a connection-wide latest-utility signal rather than per-packet semantic hooks.
- Fairness, coexistence, ablation, and figure-generation pipelines are still missing.

## Sprint 1 Vision

Sprint 1 should turn the current prototype into a usable experiment platform with clear parallel ownership.

By the end of Sprint 1, the repository should support:

- A validated Linux VPS experiment path using host-managed `tc`.
- Configurable baseline-controller runs for Quinn NewReno, Cubic, and BBR.
- Stable raw and processed result schemas that can support figures and report artifacts.
- A hardened media and semantic preprocessing pipeline.
- A documented and runtime-proven AMC preview integration path that can be extended into fuller controller work.

Sprint 1 does not need to finish the final AMC controller, but it must remove ambiguity about where that controller will live, how it will integrate, and how it will be evaluated.

## Architectural Constraints

- Preserve the current framing: this project is a semantic-aware transport policy with a congestion-control core, not a BBRv2 replacement.
- Preserve comparability: baseline and AMC runs must use the same workloads and scenario definitions.
- Keep the primary evaluation path on QUIC streams for both VOD and live traffic.
- Keep host-managed Linux `tc` as the default shaping model.
- Keep offline preprocessing and replay manifests as the runtime media abstraction.
- Keep codec-agnostic semantic inputs as the control interface.

## Parallel Workstreams

Sprint 1 is split into six workstreams plus this anchor document:

1. `10-vps-architecture.md`
2. `20-baseline-controller-integration.md`
3. `30-amc-controller-path.md`
4. `40-analysis-and-metrics.md`
5. `50-media-and-semantics.md`
6. `60-docs-and-reporting.md`

The master and worker orchestration model for these workstreams is documented in `70-agent-automation-flow.md`.

Wave A can run in parallel:

- VPS architecture
- baseline controller integration
- analysis and metrics
- media and semantics
- docs and reporting

Wave B has now landed the first AMC preview path and local runtime proof artifacts.

The next worker wave should focus on:

- Linux VPS impaired-path validation
- doc consolidation around the new AMC preview and result-schema note
- deciding whether Sprint 1 needs explicit `new_reno` evidence before exit

## Integration Checkpoints

### Checkpoint A: Shared scaffolding

Before AMC controller work starts integrating deeply, the team must reconcile:

- controller-selection config shape
- scenario config expectations
- raw-report schema
- processed-summary schema
- host-runner contract with `analyze-suite`

### Checkpoint B: Sprint exit

Sprint 1 is complete when:

- the Linux VPS host-runner path is validated for at least one impaired scenario
- baseline controller selection is runnable through config
- processed summaries are reproducible from raw reports
- the AMC controller integration path is documented and backed by at least one runtime proof artifact

## Agent Automation

Sprint execution can now be driven through workspace custom agents:

- `.github/agents/sprint-master.agent.md` orchestrates the sprint
- `.github/agents/sprint-worker.agent.md` executes bounded delegated packages
- `.github/agents/sprint-qa.agent.md` validates completed slices after worker waves finish
- `.github/prompts/run-sprint-master.prompt.md` provides a slash-command style entry point

The master/worker/QA coordination contract is defined in `70-agent-automation-flow.md`.

## Shared References

- `README.md`
- `docs/design.md`
- `docs/methodology.md`
- `docs/evaluation.md`
- `docs/replay-semantics.md`
- `crates/amc-core/src/semantics.rs`
- `crates/amc-core/src/policy.rs`
- `crates/demo-client/src/lib.rs`
- `crates/demo-server/src/lib.rs`
- `crates/harness/src/main.rs`
- `crates/harness/src/config.rs`
- `crates/harness/src/analysis.rs`
- `crates/harness/src/network.rs`
- `configs/harness/demo_vod_live.json`
- `configs/harness/vps_demo_vod_live.json`
- `scripts/media/build_replay_manifest.py`
- `scripts/experiments/run_linux_vps_suite.sh`
- `compose.yaml`