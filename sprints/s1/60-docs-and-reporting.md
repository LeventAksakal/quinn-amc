# Sprint 1 Workstream: Docs and Reporting

## Objective

Keep repository documentation aligned with implementation while Sprint 1 lands multiple parallel changes, and make the experiment path legible to future reviewers.

## Audit Status

Current repo state:

- README and the core design, methodology, evaluation, and replay-semantics notes already describe the present architecture with reasonable fidelity.
- Sprint planning artifacts exist for all six workstreams plus automation flow.
- The Sprint 1 anchor document had drifted behind the repo by underreporting already-landed analysis and media/semantics work.

Remaining gaps:

- Absorb the new `amc_preview` controller path and Windows validation caveats into the non-sprint docs where useful.
- Keep the execution model synchronized with actual worker packages and blockers.

Wave A and Wave B update:

- Sprint 1 checkpoint status is now captured in the sprint docs and dated audit note.
- The result-schema note now exists in repo docs outside the sprint artifacts.
- The dated log now captures both the local multi-controller evidence slice and the local AMC preview runtime-proof slice.

Exit blocker:

- This workstream is only complete when the sprint docs, README, and result-contract notes all match the code that exits the current validation wave.

## In Scope

- README alignment.
- Methodology and evaluation updates.
- Replay-semantics and design-note maintenance.
- Metrics-schema or result-contract documentation if added.
- Sprint-facing status updates when integration checkpoints are reached.

## Out of Scope

- Writing the paper.
- Implementing code-only features owned by other workstreams.
- Replacing the project framing with a different research claim.

## Implementation Tasks

1. Track changes from workstreams 10, 20, 30, 40, and 50.
2. Update README when experiment entry points or architecture details change.
3. Update methodology and evaluation docs when scenario or metrics assumptions change.
4. Update replay-semantics and design docs when semantic inputs or controller boundaries change.
5. Add focused documentation for any new config schema or metrics schema.
6. Review for drift between docs and implementation at each integration checkpoint.

## Critical Files and Symbols

- `README.md`
- `docs/core-idea.md`
- `docs/design.md`
- `docs/methodology.md`
- `docs/evaluation.md`
- `docs/replay-semantics.md`
- `.github/copilot-instructions.md`
- `.github/logs/`

## Dependencies

- Runs in parallel with every other workstream.
- Must track schema and contract changes from workstreams 20, 40, and 50 especially closely.

## Validation

- A fresh reviewer can identify how to run local and VPS experiments.
- Docs reflect the current config schema and result artifacts.
- No major architectural claim in docs contradicts the codebase.

## Integration Risks

- Documentation lag while multiple agents land changes.
- Methodology drifting away from what the harness can actually run.
- Missing updates when schemas evolve.

## Handoff Criteria

- Sprint 1 doc updates are merged with implementation changes.
- Drift is called out quickly when parallel work introduces conflicts.
- The repo remains readable as a research artifact, not just a code dump.

Current handoff note:

- Sprint artifacts and the result-schema note are aligned with the new AMC preview path. The remaining likely doc delta is README or design/evaluation phrasing if Sprint 1 wants the preview controller called out outside sprint docs.