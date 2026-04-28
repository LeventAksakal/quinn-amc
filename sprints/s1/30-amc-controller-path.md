# Sprint 1 Workstream: AMC Controller Path

## Objective

Define and begin implementing the first constrained integration path for a semantic-aware Quinn congestion controller that can consume utility signals without destabilizing the existing experiment platform.

## Audit Status

Current repo state:

- `amc-core` already contains codec-agnostic semantic structures and a default utility scorer.
- The harness can compute per-unit and aggregate utility offline from replay-manifest semantics and raw transfer reports.
- A live `amc_preview` controller now exists through Quinn's public `Controller` and `ControllerFactory` hooks.
- `demo-client` computes sender-visible runtime utility before writes and shares a live utility signal with the AMC preview controller.
- Raw and processed artifacts can now carry AMC runtime telemetry, including per-observation utility details and per-run min/max sample summaries.

Remaining gaps:

- The current utility feed is connection-wide and based on the latest sender utility sample, not per-packet or per-stream metadata.
- The AMC preview has local runtime proof on Windows loopback, but it still lacks impaired Linux-path evaluation.
- Future controller work still needs a clearer boundary for richer sender state if the public Quinn API remains this coarse.

Exit blocker:

- The workstream 20 scaffold dependency is cleared. The remaining blocker is deciding how much further Sprint 1 should push beyond the current preview controller.

## In Scope

- Quinn controller hook audit and integration design.
- Mapping `amc-core` utility inputs to transport-facing decisions.
- Congestion-state plumbing required for AMC decisions.
- A first constrained milestone for AMC controller behavior.
- Minimal scaffolding for future transport-policy iteration.

## Out of Scope

- Claiming paper-grade controller quality in Sprint 1.
- Full fairness or coexistence evaluation.
- Forking Quinn unless public API insufficiency is proven.

## Implementation Tasks

1. Confirm the exact Quinn `Controller` and `ControllerFactory` hooks available in the pinned dependency.
2. Define a small AMC controller milestone that is realistic for Sprint 1.
3. Identify what runtime state must flow from the sender path into utility scoring.
4. Design the boundary between manifest semantics, utility score, and transport decision.
5. Add controller-selection compatibility with workstream 20.
6. Implement the minimum viable AMC controller skeleton.
7. Add targeted tests or harness-visible evidence for decision flow.

Current implementation status:

- Completed through a minimal live preview controller that adjusts congestion growth and loss reduction from sender-computed utility.
- Completed targeted unit coverage in `amc-core` plus one local raw/processed proof run for `amc_preview`.
- Remaining follow-up is validation depth and possible refinement of the runtime signal boundary.

## Critical Files and Symbols

- `crates/amc-core/src/lib.rs`
- `crates/amc-core/src/semantics.rs`
- `crates/amc-core/src/policy.rs`
- `crates/demo-client/src/lib.rs`
- `crates/demo-server/src/lib.rs`
- `crates/harness/src/analysis.rs`
- `docs/design.md`
- `docs/evaluation.md`

## Dependencies

- Depends on workstream 20 for shared controller-selection scaffolding.
- Must align with workstream 40 on how AMC decisions are surfaced in outputs.
- May need updated semantic hints or profiles from workstream 50.

## Validation

- Compile-time integration through Quinn's public hooks.
- At least one controlled run that shows AMC controller selection and observable decision flow.
- Clear evidence that the AMC path uses semantic or utility information rather than acting as a renamed baseline.

Current validation status:

- Compile-time integration is complete.
- Targeted unit tests cover runtime utility snapshots plus utility-dependent growth and loss response.
- A local `amc_preview` run has produced raw and processed artifacts with populated runtime utility telemetry and AMC-specific summary fields.

## Integration Risks

- Public Quinn hooks may not be sufficient for the intended decision model.
- Utility inputs may be available in analysis but not yet in the live transport path.
- Overly ambitious controller behavior could destabilize Sprint 1.

## Handoff Criteria

- The AMC controller entry path is implemented or clearly stubbed in code.
- Required runtime state and future extension points are documented.
- No Quinn fork is introduced without proof that the public API is insufficient.

Current handoff note:

- This workstream is no longer design-prep only. It now has an integrated preview path with one proven runtime evidence slice, while broader evaluation remains open.