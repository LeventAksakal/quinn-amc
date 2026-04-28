# Sprint 1 Workstream: VPS Architecture

## Objective

Validate and harden the single-host Linux VPS execution path so experiment runs can be orchestrated reliably with host-managed `tc` and containerized client and server endpoints.

## Audit Status

Current repo state:

- `scripts/experiments/run_linux_vps_suite.sh` exists and already covers build, server startup, host-veth discovery, `tc` application, client execution, cleanup, and offline `analyze-suite` invocation.
- `compose.yaml` and the Dockerfiles match the intended single-host container topology.
- `crates/harness/src/network.rs` also contains a Linux-only `tc` path for local harness-driven runs, which means the repo currently has two shaping entry paths.

Remaining gaps:

- No checked-in evidence that the VPS runner has completed successfully on Linux for an impaired scenario.
- Linux runtime evidence is still missing because validation in this session was limited to shell syntax on Windows.

Exit blocker:

- The workstream cannot be marked complete until one impaired Linux run is validated and its operational contract is documented against the current script behavior.

Wave A update:

- Runner logging, readiness checks, interface validation, and cleanup handling were tightened in the first worker wave.

## In Scope

- Host-side experiment lifecycle for each run.
- Container startup, readiness checks, and cleanup.
- Host-veth discovery and `tc` application.
- Failure handling and idempotent cleanup.
- Contract between the host runner and harness `analyze-suite`.
- Scenario execution against `configs/harness/vps_demo_vod_live.json`.

## Out of Scope

- Implementing the AMC controller.
- Redesigning the harness analysis model.
- Reworking the media pipeline.

## Implementation Tasks

1. Audit `scripts/experiments/run_linux_vps_suite.sh` against the current compose and container assumptions.
2. Define the exact per-run lifecycle: build, start server, wait for readiness, apply `tc`, run client, clear `tc`, collect outputs, analyze.
3. Harden host-veth discovery across Docker/runtime variations.
4. Add explicit logging for resolved interfaces, active `tc` parameters, and cleanup success.
5. Verify failure paths clear `qdisc` state and stop containers.
6. Validate at least one impaired Linux scenario end to end.
7. Document the operational contract for later experiment automation.

## Critical Files and Symbols

- `scripts/experiments/run_linux_vps_suite.sh`
- `compose.yaml`
- `docker/demo-client.Dockerfile`
- `docker/demo-server.Dockerfile`
- `docker/harness.Dockerfile`
- `docker/harness-tc.Dockerfile`
- `configs/harness/vps_demo_vod_live.json`
- `crates/harness/src/main.rs`
- `crates/harness/src/network.rs`

## Dependencies

- Uses the current harness config schema.
- Must align with any baseline-controller config additions from workstream 20.
- Must keep raw-result output paths stable for workstream 40.

## Validation

- Run one VPS suite on Linux with at least one non-local scenario.
- Verify `tc` is applied and cleared on the intended interface.
- Verify raw reports are produced and `analyze-suite` completes.
- Verify failure cleanup leaves no stale `qdisc` state.

## Integration Risks

- Fragile interface discovery across Docker or kernel variants.
- Stale `tc` state after interrupted runs.
- Drift between VPS config schema and runner expectations.

## Handoff Criteria

- Host-runner flow is documented and repeatable.
- At least one impaired run is validated.
- Interface-discovery and cleanup logic are explicit and observable in logs.