# Sprint 2 Workstream: AMC Extension Boundary

## Objective

Document and harden the next-step boundary for `amc-core` so the current preview controller can be evaluated honestly and extended safely after VPS readiness work lands.

## Current Limitation

- The live signal exposed to the controller is still the latest connection-wide sample.
- That is enough for a preview path, but not enough to claim richer semantic-aware transport behavior.

## Sprint 2 Role

This is a follow-up workstream, not part of the first parallel wave.

It should start only after the runtime and harness contracts from Wave A are integrated.

## Acceptance Criteria

- the `amc-core` extension surface is documented in code or sprint artifacts
- experiment-time constraints of the preview controller are explicit
- future sender-state expansion points are clearer without forcing a Quinn fork decision now

## File Boundaries

- `crates/amc-core/src/lib.rs`
- `crates/amc-core/src/policy.rs`
- `crates/amc-core/src/semantics.rs`
