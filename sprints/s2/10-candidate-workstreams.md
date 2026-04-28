# Sprint 2: Candidate Workstreams

## Purpose

This document captures candidate workstream shapes for the next sprint planning conversation.

These buckets have now been narrowed into the approved Sprint 2 execution shape.

## Selected Workstream A: Online Semantics And Replay Preflight

Focus:

- `demo-client` runtime semantic alignment with replay-manifest hints
- replay-input preflight before live transfer starts
- narrowing the gap between online and offline AMC meaning

## Selected Workstream B: Raw Report Provenance

Focus:

- stronger raw-report self-description for later VPS collection
- less dependence on filename-only joins during reconciliation
- safer schema evolution ahead of Linux runs

## Selected Workstream C: Harness VPS Readiness

Focus:

- stronger suite preflight validation
- clearer Linux-only error surfaces
- lightweight export preparation for later VPS comparison work

## Sequenced Follow-Up: AMC Extension Boundary

Focus:

- clarify the honest limits of the preview controller
- make post-VPS controller iteration easier without forcing a Quinn fork decision

## Planning Notes

- The first worker wave should run `demo-client`, `demo-server`, and `harness` in parallel because their file boundaries do not overlap.
- `amc-core` follow-up should stay sequenced after Wave A integration because it depends on the runtime and artifact contracts stabilizing.