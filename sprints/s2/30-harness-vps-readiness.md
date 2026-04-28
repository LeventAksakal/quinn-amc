# Sprint 2 Workstream: Harness VPS Readiness

## Objective

Make `harness` fail early on bad configs and emit artifacts that are easier to use once real Linux VPS runs start.

## In Scope

- suite config preflight validation
- early detection of missing manifests, missing raw reports, and Linux-only scenario misuse
- export preparation for later plotting or review workflows
- stronger reconciliation checks between config intent and produced artifacts

## Out Of Scope

- actually running Linux VPS tests on this host
- host-side shell runner changes
- controller design changes in `amc-core`

## Acceptance Criteria

- invalid harness inputs fail before long-running work starts
- Linux-only scenario misuse is reported clearly
- processed outputs are easier to consume for later VPS comparison work
- no regression to local `analyze-suite` behavior

## File Boundaries

- `crates/harness/src/config.rs`
- `crates/harness/src/analysis.rs`
- `crates/harness/src/main.rs`
- `crates/harness/src/network.rs`

## Dependencies

- If export shape changes, it must not break the current JSON summary contract
- If new checks depend on raw report metadata, sequence after any report-schema integration
