# Sprint 2: Crate Gap Map

## Purpose

This document records the crate-level gaps that still stand between the current repo and low-friction Linux VPS testing.

## `amc-core`

Current strength:

- semantic structures, scoring, and preview controller behavior already exist

Missing gaps:

- no explicit tuning surface for experiment-time controller calibration
- runtime signal boundary remains connection-wide and last-sample based
- future richer sender state is not yet codified as a stable extension contract

Sprint value:

- medium for immediate VPS readiness
- high for the first post-VPS controller iteration

## `demo-client`

Current strength:

- config-driven replay, baseline selection, and runtime utility telemetry are already implemented

Missing gaps:

- runtime utility still ignores replay-manifest semantic hints used by offline analysis
- replay inputs are not fully preflighted before connect
- later VPS debugging will be harder if online utility meaning remains only heuristic and implicit

Sprint value:

- high for immediate VPS readiness

## `demo-server`

Current strength:

- raw transfer reports and runtime telemetry capture already exist

Missing gaps:

- raw report self-description is thin for later VPS reconciliation
- report provenance still leans on path and harness context rather than stronger embedded metadata
- schema evolution risk is higher than needed before Linux collection begins

Sprint value:

- high for immediate VPS readiness

## `harness`

Current strength:

- suite orchestration, offline analysis, and summary writing already exist

Missing gaps:

- config and artifact preflight validation are too weak for later VPS usage
- Linux-only assumptions are not surfaced early enough
- export surfaces for comparison and plotting are still shallow

Sprint value:

- highest for immediate VPS readiness

## Priority Order

1. `harness`
2. `demo-client`
3. `demo-server`
4. `amc-core`

## Worker Mapping

- Worker A: `demo-client` online semantics and replay preflight
- Worker B: `demo-server` raw-report provenance
- Worker C: `harness` preflight validation and export preparation
- Later Worker D: `amc-core` extension-boundary follow-up after Wave A integration
