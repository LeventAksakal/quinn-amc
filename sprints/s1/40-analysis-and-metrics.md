# Sprint 1 Workstream: Analysis and Metrics

## Objective

Stabilize the raw-to-processed results pipeline so the repo can produce reproducible summary artifacts and figure-ready data across baseline and future AMC runs.

## Audit Status

Current repo state:

- Raw transfer reports are emitted by `demo-server` with segment observations and a transfer summary.
- The harness writes suite summaries and per-run AMC analysis JSON.
- Sample artifacts already exist under `results/raw/` and `results/processed/harness/`.
- Controller identity is now recorded in raw transfer summaries, suite summaries, and per-run AMC analysis outputs.

Remaining gaps:

- Fairness/coexistence metrics and figure-ready exports are not implemented.
- Figure-ready export contracts beyond the current JSON artifacts are still not explicit.

Exit blocker:

- The original schema-documentation blocker is cleared. Remaining work is expansion, not minimum-contract definition.

## In Scope

- Raw report schema review and hardening.
- Processed summary schema review and hardening.
- Baseline-controller labeling in analysis outputs.
- Multimedia and fairness-oriented metric definitions for Sprint 1 outputs.
- Figure-ready export planning or implementation.

## Out of Scope

- Full paper figure production.
- Implementing the transport controller itself.
- Linux VPS orchestration changes outside what is required for stable outputs.

## Implementation Tasks

1. Audit existing raw report fields emitted by demo-server.
2. Audit processed summary fields emitted by harness analysis.
3. Define the minimum stable schema needed for Sprint 1 comparisons.
4. Add controller identity, scenario identity, and run identity where missing.
5. Add fairness and coexistence placeholders or initial metrics if enough data exists.
6. Add a figure-ready export path or a clear contract for later plotting scripts.
7. Document metric meanings and units.

## Critical Files and Symbols

- `crates/demo-server/src/lib.rs`
- `crates/harness/src/analysis.rs`
- `crates/harness/src/main.rs`
- `results/raw/`
- `results/processed/harness/`
- `docs/methodology.md`
- `docs/evaluation.md`

## Dependencies

- Must absorb controller-selection changes from workstream 20.
- Must leave room for AMC decision outputs from workstream 30.
- Must preserve compatibility with current raw reports where reasonable.

## Validation

- Existing sample reports still parse.
- Processed summaries remain reproducible.
- New fields are populated in at least one local run.

Current validation status:

- The explicit Sprint 1 result-schema note now exists in `docs/result-schema.md`.
- Local baseline and AMC preview artifacts both populate the controller and runtime-summary fields now used for reconciliation.

## Integration Risks

- Schema drift between producer and analyzer.
- Metric names without stable definitions.
- Over-design before enough controller variety exists.

## Handoff Criteria

- Sprint 1 outputs have an explicit minimal schema.
- Controller and scenario identity are represented consistently.
- Future plotting work has a clear contract to build on.

Current handoff note:

- The minimum Sprint 1 schema contract is now present. Remaining work is additional metrics and export depth rather than schema ambiguity.