# Sprint 2 Workstream: Online Semantics and Provenance

## Objective

Reduce mismatch between live runtime behavior and offline analysis, and make raw report artifacts more trustworthy before Linux VPS collection starts.

## Scope Split

### Slice A: `demo-client`

- consume replay-manifest semantic hints where practical in runtime utility telemetry
- preflight manifest and referenced segment artifacts before opening the QUIC connection

### Slice B: `demo-server`

- enrich raw report metadata so later VPS collection is easier to reconcile without relying only on file naming and external config joins

## Out Of Scope

- Linux host-runner changes
- fairness/coexistence implementation
- redesigning Quinn controller hooks beyond the current preview boundary

## Acceptance Criteria

- `demo-client` runtime utility is less heuristic-only and closer to the replay-manifest contract already used by `harness`
- `demo-client` fails early on obviously broken replay inputs
- `demo-server` raw reports carry clearer provenance or schema self-description for later collection review
- existing client/server replay behavior still compiles cleanly

## File Boundaries

- `crates/demo-client/src/lib.rs`
- `crates/demo-server/src/lib.rs`

## Dependencies

- Any raw schema additions must remain backward-compatible for `harness`
- `demo-client` and `demo-server` changes should avoid shared protocol churn unless the benefit is clear
