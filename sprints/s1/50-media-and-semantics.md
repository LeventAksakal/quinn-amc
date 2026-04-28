# Sprint 1 Workstream: Media and Semantics

## Objective

Harden the offline media pipeline and replay-manifest semantics so Sprint 1 experiments run against reliable artifacts and a clear semantic contract.

## Audit Status

Current repo state:

- Download, preprocessing, and replay-manifest generation scripts exist.
- Checked-in manifests contain semantic defaults and per-segment hints.
- The harness already prefers replay-manifest semantics over fallback profile values when available.

Remaining gaps:

- Full regeneration evidence with real toolchain execution is still missing from this sprint pass.
- Regeneration has not been turned into a repeatable validation step for the sprint.
- The semantic contract is documented, but enforcement is still soft.

Exit blocker:

- The workstream should remain open until at least one regeneration path fails clearly on bad inputs and still produces a manifest consumable by the client and harness on valid inputs.

Wave A update:

- The preprocessing path now fails early on malformed MPDs, missing init/media outputs, zero-byte segments, non-contiguous numbering, and naming drift.

## In Scope

- Media download and preprocessing reliability.
- Replay-manifest validation and consistency checks.
- Semantic-hint generation quality.
- Clear mapping between preprocessing artifacts and runtime semantics.
- Better defaults or richer hints where they improve reproducibility.

## Out of Scope

- Runtime congestion-control implementation.
- Major changes to the harness orchestration model.
- Full media-stack realism beyond the current replay approach.

## Implementation Tasks

1. Audit download and preprocessing scripts against current manifest expectations.
2. Validate replay-manifest schema and required fields.
3. Confirm semantic defaults and per-segment hints are consumed consistently downstream.
4. Improve preprocessing-side validation so broken manifests fail early.
5. Refine heuristic semantic rules only where they clearly improve experiment quality.
6. Document any semantic schema changes in lockstep with docs.

## Critical Files and Symbols

- `scripts/media/download_open_media.sh`
- `scripts/media/preprocess_streams.sh`
- `scripts/media/build_replay_manifest.py`
- `data/processed/manifests/`
- `docs/replay-semantics.md`
- `crates/demo-client/src/lib.rs`
- `crates/harness/src/analysis.rs`

## Dependencies

- Must preserve the replay-manifest contract used by demo-client.
- Must align semantic changes with workstream 30 if AMC integration needs richer hints.
- Must align documentation updates with workstream 60.

## Validation

- Regenerate at least one manifest successfully.
- Verify demo-client can still consume the regenerated manifest.
- Verify harness analysis still interprets semantic hints correctly.

## Integration Risks

- Manifest schema drift across scripts and Rust structs.
- Semantic hints becoming less codec-agnostic over time.
- Improved heuristics breaking comparability with earlier artifacts.

## Handoff Criteria

- Preprocessing is reliable enough for Sprint 1 runs.
- Manifest expectations are explicit.
- Semantic-hint generation is documented and validated.