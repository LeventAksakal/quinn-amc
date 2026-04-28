# Sprint 2: Status and Vision

## Purpose

This document is the shared baseline for Sprint 2. It turns the current repo audit into an execution-oriented plan focused on crate-level readiness for later Linux VPS testing.

## Sprint 2 Objective

Make the Rust crates operationally ready for real VPS validation without pretending that Linux-path evidence already exists.

This sprint is about removing crate-level ambiguity and contract gaps so the later VPS phase becomes a validation exercise, not a design-discovery exercise.

## Current State At Sprint Start

What already exists:

- `amc-core` exposes codec-agnostic semantics, utility scoring, and the `amc_preview` controller path.
- `demo-client` can replay preprocessed media artifacts over QUIC with baseline-controller selection and optional runtime utility telemetry.
- `demo-server` records raw segment observations and transfer summaries.
- `harness` can run local suites, analyze existing raw reports, and emit processed summaries plus per-run AMC analysis.

What is still missing before VPS work is low-friction:

- online AMC runtime semantics in `demo-client` still use local heuristics instead of replay-manifest semantic hints, which risks divergence from offline analysis
- raw report provenance is still thin for later VPS reconciliation
- `harness` does not yet perform strong preflight validation of suite inputs and Linux-only assumptions
- figure-ready and reviewer-friendly export surfaces are still shallow
- `amc-core` still exposes only a constrained preview controller boundary

## Crate-Level Gap Summary

### `amc-core`

- The preview controller is real, but its live signal boundary is still connection-wide and coarse.
- Utility behavior is tested locally, but the tuning and extension surface for future VPS experiments is still narrow.

### `demo-client`

- Runtime utility telemetry is derived from sender-side heuristics instead of the replay-manifest semantic hints already used by offline analysis.
- Replay inputs are validated incrementally during transfer rather than aggressively preflighted before the connection starts.

### `demo-server`

- Raw reports are usable, but they are not yet rich enough for future VPS reconciliation without joining against external config and runner context.
- The raw schema needs clearer self-description for multi-run Linux collection.

### `harness`

- Config parsing exists, but stronger preflight and contract validation are still missing.
- Offline analysis exists, but export surfaces for later plotting and run comparison are still limited.

## Sprint 2 Workstreams

Sprint 2 is organized around crate-level readiness rather than Sprint 1's broader architecture buckets:

1. `10-crate-gap-map.md`
2. `20-online-semantics-and-provenance.md`
3. `30-harness-vps-readiness.md`
4. `40-amc-extension-boundary.md`

## Sequencing

Wave A can run in parallel:

- `demo-client` online semantic alignment and replay preflight hardening
- `demo-server` raw-report provenance hardening
- `harness` suite preflight and export preparation

Wave B should be sequenced after Wave A integration:

- `amc-core` extension-boundary hardening based on the landed runtime and harness contracts

## Wave A Status

Wave A is now implemented and compile-validated on three non-overlapping slices:

- `demo-client`: runtime utility now consumes replay-manifest semantics when available and preflights replay inputs before connection setup
- `demo-server`: raw transfer reports now include additive provenance metadata for later VPS reconciliation
- `harness`: suite configs now fail earlier and processed outputs now include an additive comparison export sidecar

Scoped QA passed for all three slices at the crate compile level.

Current residual risk:

- the new behavior is compile-validated and code-reviewed, but not yet runtime-validated through a fresh end-to-end transfer on this turn
- the new raw report metadata and comparison export are additive, but downstream docs and explicit schema notes still need reconciliation in a later slice
- `amc-core` still remains on the sequenced follow-up path rather than entering the first worker wave

## Sprint 2 Exit Target

Sprint 2 is successful when all of the following are true:

- online and offline semantic inputs are closer to one shared contract
- raw reports are easier to reconcile after future VPS runs
- harness configs fail early on missing or Linux-only prerequisites
- the remaining `amc-core` limitations are explicit enough that VPS testing will evaluate a known preview boundary

Sprint 2 does not require final Linux VPS evidence. It requires the codebase to be ready for that evidence phase.

## Constraints

- Do not treat WSL as primary Linux validation evidence.
- Preserve comparability across baseline and AMC runs.
- Keep the primary path on QUIC streams.
- Keep host-managed Linux `tc` as the intended shaping model.
- Keep crate-level changes focused and avoid schema churn across multiple workers at once.

## Shared References

- `README.md`
- `docs/design.md`
- `docs/methodology.md`
- `docs/evaluation.md`
- `docs/result-schema.md`
- `sprints/s1/80-audit-2026-04-28.md`
- `crates/amc-core/src/policy.rs`
- `crates/demo-client/src/lib.rs`
- `crates/demo-server/src/lib.rs`
- `crates/harness/src/config.rs`
- `crates/harness/src/analysis.rs`
- `crates/harness/src/main.rs`
