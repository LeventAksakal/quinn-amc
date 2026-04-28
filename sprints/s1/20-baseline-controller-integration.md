# Sprint 1 Workstream: Baseline Controller Integration

## Objective

Expose Quinn baseline-controller selection through config and runtime wiring so NewReno, Cubic, and BBR runs are comparable under the same workloads and scenarios.

## Audit Status

Current repo state:

- `RunConfig` now carries a controller field with a default baseline selection.
- `demo-client` now uses Quinn public congestion-controller factories through `TransportConfig::congestion_controller_factory(...)`.
- Raw and processed outputs now include controller identity.
- Local loopback evidence now exists for at least two distinct baseline controllers (`cubic` and `bbr`).

Remaining gaps:

- Capture local or Linux evidence for `new_reno` if Sprint 1 needs all three baselines exercised explicitly before exit.
- Resolve the Windows `run-suite` completion issue so multi-controller evidence does not require the current offline `analyze-suite` workaround.

Exit blocker:

- The main code-path blocker is cleared. Remaining work is validation quality and broader execution evidence, not controller wiring.

Wave A update:

- A shared `BaselineController` enum now exists.
- Local and VPS harness configs now declare `controller` explicitly.
- Controller identity is enforced against raw report provenance during harness processing.

## In Scope

- Controller-selection schema in harness or demo-client config.
- Runtime wiring into Quinn transport configuration.
- Baseline run labeling in raw and processed artifacts.
- Local validation of baseline-controller selection before Linux VPS runs.

## Out of Scope

- Final AMC controller behavior.
- Fairness/coexistence logic beyond baseline labeling and execution support.
- Major redesign of the replay sender.

## Implementation Tasks

1. Audit Quinn's public congestion-controller factory interfaces used by the current dependency version.
2. Add a shared controller-selection enum or config field.
3. Extend harness run config to carry controller identity.
4. Wire demo-client transport config to instantiate the selected baseline controller.
5. Propagate controller identity into raw reports and processed outputs.
6. Validate that identical runs can be executed under NewReno, Cubic, and BBR.
7. Ensure config errors fail clearly when an unsupported controller is requested.

## Critical Files and Symbols

- `crates/demo-client/src/lib.rs`
- `crates/harness/src/config.rs`
- `crates/harness/src/main.rs`
- `crates/harness/src/analysis.rs`
- `configs/harness/demo_vod_live.json`
- `configs/harness/vps_demo_vod_live.json`
- `Cargo.toml`

## Dependencies

- Must preserve current replay and report behavior.
- Must align with workstream 40 on output schema changes.
- Provides the controller-selection scaffolding needed by workstream 30.

## Validation

- Run the same local workload under at least two baseline controllers.
- Verify controller identity is visible in outputs.
- Verify no regressions to default replay behavior.

Current validation status:

- Satisfied for `cubic` and `bbr` on local Windows loopback.
- Still pending for `new_reno` and for impaired Linux-host validation.

## Integration Risks

- Quinn API mismatch or feature assumptions.
- Controller-selection schema drift between config and runtime.
- Silent fallback to a default controller if wiring is incomplete.

## Handoff Criteria

- Controller selection is config-driven.
- Baseline runs are reproducible.
- Output artifacts explicitly identify the baseline controller used.

Current handoff note:

- This workstream is effectively integrated for Sprint 1 Wave B, with remaining risk concentrated in execution reliability rather than code-path completeness.