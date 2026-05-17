# Project Completion Plan

Date: 2026-05-17

## Purpose

This document is the top-level roadmap from the current repository state to a finished research artifact.

It is intentionally phase-gated and sequential. Each phase is meant to be planned and implemented on its own to avoid context rot, accidental scope expansion, and hand-wavy execution.

The intended workflow is simple:

1. Assign one phase to one agent.
2. That agent produces a detailed implementation plan for that phase only.
3. Execute that phase.
4. Re-check repository state.
5. Move to the next phase.

Do not let an agent plan multiple phases at once unless the current phase explicitly requires it.

## Definition Of 100% Completion

For this repository, 100% completion means:

- the benchmark scope is frozen and explicitly documented
- the canonical VPS and coexistence workflows are consistent across docs and tooling
- the final evidence set is reproducible from documented inputs and commands
- the AMC claim is bounded, explicit, and matched to the evidence actually collected
- the final benchmark artifacts are frozen and tied to exact configs
- the full required figure set exists and is publication-ready
- a report package can be generated from the frozen artifacts
- a ratatui live-demo exists for a single chosen run and makes the controller internals understandable
- final validation passes and the repo records what is complete versus what remains future work

## Phase 1 Freeze Decisions

Phase 1 is resolved around the current repository boundary rather than a larger redesign.

- AMC scope: repository completion stops at AMC v1. The final claim must stay bounded to the current latest-sample, connection-wide runtime signal model. AMC v2 remains explicit future work outside this completion plan.
- Workload framing: live is the primary claim surface. VOD remains required supporting evidence for boundedness and comparability, but completion does not require AMC to outperform the baselines on VOD startup behavior.
- Reproducibility target: one canonical VPS evidence workflow plus required local parity validation. Final reported evidence comes from the VPS path. Local configs remain required smoke and regression coverage so the repo can be exercised without the VPS.
- Mandatory evidence families:
   - `configs/harness/vps_fixed_preset_controller_matrix.json` as the primary fixed-preset workload matrix
   - `configs/harness/vps_host_live_coexistence_bbr_guardrail.json` as the required fairness and coexistence guardrail suite until the legacy docker runner gains coexistence support
   - `configs/harness/local_controller_matrix.json` and `configs/harness/local_live_immediate_amc_bbr_coexistence.json` as required local parity coverage
- Support-only or exploratory configs: `vps_baseline_vod_live.json` and `vps_demo_vod_live.json` remain workflow-validation suites; `vps_live_realtime_controller_matrix.json`, `vps_lte_constrained_live_matrix.json`, and `vps_live_coexistence_bbr_guardrail.json` remain exploratory or non-canonical until later phases promote them explicitly.
- Mandatory final deliverables:
   - figure families covering live usefulness and freshness, delivery latency, jitter, throughput, VOD startup and rebuffer behavior, and fairness share plus Jain fairness
   - a report package with frozen configs, commands, processed outputs, figures, bounded interpretations, methodology, limitations, and reproducibility notes
   - a single-run ratatui introspection demo driven by one frozen raw report
- Out of scope for repository completion unless a later phase reopens the decision: AMC v2 state expansion, QUIC datagrams in the primary claim, dynamic adaptation suites inside the main matrix, and a comparative multi-controller demo lab

## Phase 4 Resolution

Phase 4 resolves the controller milestone by freezing repository completion at AMC v1.

- Repository completion does not require widening `RuntimeUtilityState` beyond the latest connection-wide utility sample.
- Controller completion means the repo can support a bounded AMC v1 claim from the frozen evidence, not that AMC must beat BBR on every matrix cell.
- The required live-primary claim is now: AMC v1 shows bounded benefit against the loss-based baselines on the hardest constrained live presets while remaining fairness-safe against the required BBR guardrail.
- The required supporting limitation is now: BBR remains the strongest overall baseline in the fixed matrix, and AMC v1 is not a VOD startup winner.
- AMC v2 remains explicit future work for richer controller state, broader fairness coverage, and stronger freshness claims.

## Phase 5 Resolution

Phase 5 resolves the benchmark freeze boundary by naming one exact evidence set for the remaining phases.

- The frozen final-evidence set is the canonical VPS fixed-preset matrix plus the canonical VPS host fairness guardrail suite.
- The authoritative local evidence inputs are the four processed artifacts under `results/processed/harness/` for those two suites.
- Local parity, workflow-validation, and exploratory outputs remain documented support surfaces, not part of the final evidence claim.
- The evidence freeze point remains inside the Phase 4 AMC v1 claim: constrained-live gains against the loss-based baselines, fairness safety against BBR, and honest VOD limitations.

## Phase 6 Resolution

Phase 6 resolves the final figure system around the frozen Phase 5 artifacts instead of the wider `results/` tree.

- The canonical figure inputs are the two frozen comparison exports: `results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json` and `results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json`.
- The validated final figure directory is `results/figures/harness/`, generated from those two inputs with suite-prefixed file names so canonical matrix plots and fairness plots cannot overwrite each other.
- The fixed-preset matrix figure family now covers live and VOD usefulness, deadline miss rate, throughput, delivery latency, jitter, live age of information, VOD startup delay, and VOD rebuffer ratio.
- The fairness guardrail figure family now covers the same live transport and usefulness surfaces plus foreground throughput share, throughput ratio, and Jain fairness index.
- Later phases should consume the Phase 6 figure outputs directly or regenerate them only from the same two canonical comparison exports.

## Phase 7 Resolution

Phase 7 resolves the report package around one versioned Markdown report plus one validated generated package workflow.

- The canonical reviewer-readable report now lives at `docs/final-report.md` and stays versioned with the repository.
- The validated report-package command is `cargo run -p harness -- package-report`, which consumes only the two canonical comparison exports plus the frozen Phase 6 figure directory.
- The validated generated package layout lives under `results/reports/final/` and contains a packaged report copy, the four canonical processed artifacts, the 39 frozen figures, a manifest, and a reproducibility note.
- Phase 7 does not widen the evidence boundary, regenerate plots, or add HTML or PDF export; later phases should inherit the Markdown-first package workflow as-is unless they explicitly reopen format scope.

## Phase Overview

### Phase 1: Define Done And Freeze Scope

Establish the exact meaning of completion for this repo.

This phase must decide:

- which benchmark suites are required for the final artifact
- which configs are final-evidence configs versus exploratory configs
- which figures are mandatory
- what the final report package must contain
- what the live-demo must show
- whether completion means a polished AMC v1 claim or includes one explicit AMC v2 milestone

This phase blocks everything else because it defines what counts as done.

### Phase 2: Repair The Evidence Path And Sync Documentation

Make the methodology, tooling, and docs agree on the same canonical workflow.

This phase must:

- reconcile the README, methodology, copilot instructions, and VPS handoff notes
- choose the canonical remote workflow
- mark unsupported paths as unsupported rather than leaving them ambiguous
- freeze the configs that feed the final evidence package

This phase removes documentation drift and workflow ambiguity.

### Phase 3: Complete Reproducibility And Runner Coverage

Close the gap between implemented harness capability and the documented VPS workflow.

This phase must:

- make the canonical workflow capable of reproducing the required evidence families
- verify media preprocessing and replay regeneration
- add regression checks for silent validity failures such as wrong controller selection and response-delivery races
- ensure coexistence evidence is reproducible from the chosen workflow

This phase turns the workflow from "works with caveats" into a defensible reproduction path.

### Phase 4: Finalize The AMC Completion Milestone

Treat the congestion controller as a bounded milestone instead of an open-ended research thread.

This phase must:

- finalize the AMC v1 interpretation under the frozen matrix
- decide whether the repository stops at a bounded AMC v1 claim or includes one explicit AMC v2 state-expansion step
- define the exact acceptance criteria for controller completion

The output of this phase is clarity about what controller state counts as complete for this project.

### Phase 5: Close The Benchmark And Freeze Results

Run or confirm the final evidence set under the canonical workflow and freeze it.

This phase must:

- confirm the final matrix and fairness suites
- write bounded interpretations for each major result family
- freeze the raw and processed artifacts used by all downstream deliverables
- tie those artifacts to exact configs and commands

This phase creates the final evidence base.

### Phase 6: Complete The Figure System

Expand the current plotting path into the full final chart set.

This phase must:

- produce the required live, VOD, fairness, throughput, latency, jitter, and usefulness plots
- standardize naming, styling, and output conventions
- ensure the figures are fit for final reporting

This phase should consume frozen evidence only.

### Phase 7: Complete The Report Package

Create the final report-generation path and artifact structure.

This phase must:

- bind processed outputs, figures, experiment metadata, and interpretation into one report package
- include methodology, results, limitations, and reproducibility notes
- produce a reviewer-readable final deliverable

This phase turns the evidence set into a coherent artifact.

### Phase 8: Complete The Live-Demo

Turn the existing ratatui path into a final single-run controller-inspection demo.

This phase must:

- pick one final showcase run
- expose workload behavior, utility evolution, controller-visible state, and observed delivery behavior clearly
- document how to launch and use the demo
- ensure the demo consumes stable frozen artifacts rather than exploratory runs

This phase is the "gut-see the internals" deliverable.

### Phase 9: Final Integration And Release Gate

Validate the entire repository as a finished artifact.

This phase must:

- run the final build and validation checklist
- verify the benchmark path, figure path, report path, and live-demo path end to end
- archive the final deliverables
- record a final completion note plus explicit future-work items outside scope

This phase is the final closeout gate.

## Required Ordering

The default execution order is:

1. Phase 1
2. Phase 2
3. Phase 3
4. Phase 4
5. Phase 5
6. Phase 6
7. Phase 7
8. Phase 8
9. Phase 9

Phase 8 may overlap late Phase 6 or Phase 7 only after the evidence format and frozen artifact set are stable.

## Phase Inputs And Outputs

### Phase 1 Input

- current repo scope and docs
- current results and known limitations

### Phase 1 Output

- written definition of done captured in the main repo docs
- frozen config classification for final evidence, local parity, and exploratory suites
- acceptance criteria for later phases anchored to the AMC v1, live-primary, fairness-required boundary

### Phase 2 Input

- frozen scope boundary
- current workflow and documentation drift

### Phase 2 Output

- one canonical documented workflow
- frozen config set for final evidence

### Phase 3 Input

- canonical workflow
- final evidence requirements

### Phase 3 Output

- reproducible execution path
- runner coverage and regression protection

### Phase 4 Input

- frozen benchmark scope
- reproducible workflow

### Phase 4 Output

- final controller-completion definition
- bounded AMC claim

### Phase 5 Input

- final controller scope
- canonical workflow

### Phase 5 Output

- frozen final evidence set
- bounded result interpretations

### Phase 6 Input

- frozen evidence set

### Phase 6 Output

- full figure set
- standardized final plot outputs

### Phase 7 Input

- frozen evidence and figures

### Phase 7 Output

- final report package

### Phase 8 Input

- frozen evidence set
- stable data model for demo consumption

### Phase 8 Output

- documented ratatui showcase demo

### Phase 9 Input

- completed figures, report, and demo

### Phase 9 Output

- final integrated artifact
- completion note

## Guidance For Agents

When you assign a phase to an agent, the agent should:

1. Read this file first.
2. Work on one phase only.
3. Produce a detailed plan for that phase only.
4. State assumptions, risks, entry criteria, exit criteria, and verification steps.
5. Avoid redesigning later phases unless the current phase forces a dependency decision.

Recommended prompt shape:

"You own Phase X from PROJECT_COMPLETION_PLAN.md. Create a detailed execution plan for this phase only. Do not plan later phases except where dependencies force you to name them. Include scope, deliverables, risks, decisions, verification, and exact files or commands likely to be touched."

## Phase 1 Settled Decisions

These decisions are now frozen for the rest of the roadmap unless a later change explicitly reopens scope:

1. AMC scope decision
   Repository completion is a polished AMC v1 claim only. AMC v2 remains future work.

2. Reproducibility target decision
   Completion requires the canonical VPS evidence path plus local parity validation coverage.

3. Live-demo scope decision
   The required demo is a polished single-run introspection viewer, not a broader controller lab.

4. Workload priority decision
   Live traffic is the primary claim surface; VOD is required supporting evidence.

5. Fairness scope decision
   Coexistence and fairness are mandatory final evidence, but they remain a separate guardrail family rather than part of the single-flow matrix.

## Primary Repository Surfaces

- [README.md](README.md)
- [TODO.md](TODO.md)
- [docs/methodology.md](docs/methodology.md)
- [docs/evaluation.md](docs/evaluation.md)
- [docs/vps-results-handoff.md](docs/vps-results-handoff.md)
- [docs/result-schema.md](docs/result-schema.md)
- [crates/amc-core/src/lib.rs](crates/amc-core/src/lib.rs)
- [crates/amc-core/src/policy.rs](crates/amc-core/src/policy.rs)
- [crates/demo-client/src/lib.rs](crates/demo-client/src/lib.rs)
- [crates/demo-server/src/lib.rs](crates/demo-server/src/lib.rs)
- [crates/harness/src/main.rs](crates/harness/src/main.rs)
- [crates/harness/src/analysis.rs](crates/harness/src/analysis.rs)
- [crates/harness/src/plot.rs](crates/harness/src/plot.rs)
- [crates/harness/src/tui_demo.rs](crates/harness/src/tui_demo.rs)
- [scripts/experiments/run_linux_vps_suite.sh](scripts/experiments/run_linux_vps_suite.sh)
- [configs/harness](configs/harness)
- [results/processed/harness](results/processed/harness)
- [results/figures/harness](results/figures/harness)

## Completion Checkpoints

1. Scope checkpoint: completion criteria are explicit and frozen.
2. Workflow checkpoint: all docs describe the same validated path.
3. Reproducibility checkpoint: final evidence can be regenerated cleanly.
4. AMC checkpoint: the controller claim is bounded and supported.
5. Evidence checkpoint: final results are frozen and traceable.
6. Figure checkpoint: the full chart set exists and is consistent.
7. Report checkpoint: the final report package is generated successfully.
8. Demo checkpoint: the ratatui showcase run works and explains internals clearly.
9. Release checkpoint: the repo has a final integrated artifact and explicit future-work note.