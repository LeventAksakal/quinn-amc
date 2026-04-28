# Sprint 2 Automation Flow

## Purpose

This document defines the first Sprint 2 worker wave and the QA slices that must follow it.

## Wave A Worker Packages

### Worker A: `demo-client` online semantics

- Objective: align runtime utility telemetry more closely with replay-manifest semantics and add replay-input preflight checks
- Allowed files: `crates/demo-client/**`
- Out of scope: server report schema, harness exports, `amc-core` redesign
- Validation: `cargo check -p demo-client`
- Return: changed files, validation result, remaining semantic mismatches

### Worker B: `demo-server` report provenance

- Objective: improve raw report self-description for later VPS reconciliation without breaking existing consumers
- Allowed files: `crates/demo-server/**`
- Out of scope: client replay logic, harness export redesign, Linux runner changes
- Validation: `cargo check -p demo-server`
- Return: changed files, validation result, backward-compatibility notes

### Worker C: `harness` preflight and export readiness

- Objective: add stronger suite preflight checks and prepare a lighter-weight comparison export surface
- Allowed files: `crates/harness/**`
- Out of scope: demo-client runtime logic, demo-server protocol changes, host runner shell scripts
- Validation: `cargo check -p harness`
- Return: changed files, validation result, integration notes

## Wave A QA Packages

### QA A: `demo-client`

- Scope: `crates/demo-client/**`
- Acceptance criteria: compiles, preserves replay path, and clearly improves preflight or runtime semantic alignment
- Checks: `cargo check -p demo-client`

### QA B: `demo-server`

- Scope: `crates/demo-server/**`
- Acceptance criteria: compiles and raw schema changes remain backward-compatible for current consumers
- Checks: `cargo check -p demo-server`

### QA C: `harness`

- Scope: `crates/harness/**`
- Acceptance criteria: compiles, invalid inputs fail earlier, and current analysis path remains intact
- Checks: `cargo check -p harness`

## Integration Rule

- Integrate Wave A before launching any `amc-core` follow-up worker because the runtime and artifact contracts must settle first.
