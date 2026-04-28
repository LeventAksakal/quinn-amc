# Sprint 2: Planning Intake

## Status

This folder started as a planning scaffold and is now promoted into an execution-ready Sprint 2 planning package.

The current sprint direction is approved for bounded worker execution, but Sprint 2 artifacts should still be treated as living documents until the first Linux VPS evidence slice lands.

## Planning Goal

Define the next sprint after Sprint 1 using the current repository state, the latest documented Sprint 1 status, and explicit user guidance on priorities, validation targets, and acceptable scope.

Approved direction for Sprint 2:

- Skip WSL as a primary Linux validation path.
- Prepare the crates and contracts so the repository is almost ready for real VPS testing.
- Focus first on crate-level readiness gaps rather than trying to close the Linux evidence gap on this Windows host.

## Current Carry-Over From Sprint 1

Based on `sprints/s1/00-status-and-vision.md`, the most obvious carry-over items are:

- Linux VPS impaired-path validation on the host-managed `tc` path.
- Documentation consolidation around the AMC preview path and the result-schema note.
- Follow-on experiment work for fairness, coexistence, ablation, and figure-generation once the platform path is stable enough.
- A decision on whether Sprint 1 exit requires explicit `new_reno` runtime evidence in addition to the current `cubic`, `bbr`, and `amc_preview` validation slices.

These are now filtered into Sprint 2 commitments and deferred items.

Committed for Sprint 2 planning and worker execution:

- crate-level VPS-readiness hardening in `amc-core`, `demo-client`, `demo-server`, and `harness`
- stronger runtime and raw-report provenance contracts before Linux collection begins
- harness preflight validation and export preparation for future VPS result review

Explicitly deferred for now:

- using WSL as a Linux validation sign-off path
- fairness/coexistence measurement implementation beyond contract preparation
- paper-ready figure production

## Intake Questions

Resolved intake for this sprint start:

- Primary objective: platform hardening and crate-level readiness for later Linux VPS validation.
- Must land first: online semantic alignment, raw-report provenance hardening, and harness preflight checks.
- Linux validation threshold in this sprint: be operationally ready for VPS runs, not fully validated on Linux yet.
- Optimization target: reproducibility and experiment readiness, not publication polish.
- Blocking risks: schema drift across crates, runtime/offline semantic mismatch, and hidden Linux-only assumptions.

## Expected Planning Outputs

Sprint 2 planning now produces:

- a status-and-vision document for Sprint 2
- a crate-gap map and bounded workstream plans
- explicit sequencing notes for shared config or schema changes
- a clear master, worker, and QA execution outline for the first worker wave

## References

- `sprints/s1/00-status-and-vision.md`
- `sprints/s1/70-agent-automation-flow.md`
- `README.md`
- `docs/design.md`
- `docs/methodology.md`
- `docs/evaluation.md`
- `configs/harness/`
- `results/processed/harness/`