# Sprint 1 Automation Flow

## Purpose

This document defines the master, worker, and QA automation model for Sprint 1 execution.

The goal is to let one orchestration agent coordinate multiple focused worker agents, run post-wave QA validation, and preserve one integrated report back to the user.

## Agent Roles

### Master

- Agent file: `.github/agents/sprint-master.agent.md`
- Responsibility: orchestrate sprint work, create worker packages, launch workers, integrate results, launch QA, and report final status
- Model: `GPT-5 (copilot)`

### Worker

- Agent file: `.github/agents/sprint-worker.agent.md`
- Responsibility: execute one bounded work package and return a concise implementation handoff
- Model: `GPT-5 (copilot)`

### QA

- Agent file: `.github/agents/sprint-qa.agent.md`
- Responsibility: validate completed implementation slices with scoped checks and return pass or fail evidence
- Model: `GPT-5 (copilot)`

## Execution Contract

The master agent owns:

- reading the sprint plan
- deciding task boundaries
- deciding what can run in parallel
- creating explicit worker packages
- creating scoped QA packages after each worker wave completes
- resolving integration conflicts
- updating sprint artifacts when work changes the sprint state

The worker agent owns:

- one bounded task package
- local code or docs changes for that package
- requested validation for that package
- concise reporting back to the master

The QA agent owns:

- one bounded validation package
- targeted checks, tests, or static validation for that package
- pass or fail reporting against acceptance criteria
- concise reporting back to the master

Workers and QA agents do not spawn other agents. All fan-out and reconciliation stay in the master.

## Standard Flow

1. User invokes the sprint orchestration flow directly or through `.github/prompts/run-sprint-master.prompt.md`.
2. Master reads `sprints/s1/00-status-and-vision.md` and the relevant workstream plans.
3. Master creates one worker package per independent work slice.
4. Master launches one or more `Sprint Worker` subagents.
5. Workers return bounded change reports.
6. When a wave is complete, Master creates one or more scoped QA packages.
7. Master launches `Sprint QA` subagents against the completed slices.
8. QA agents return pass or fail reports.
9. Master resolves conflicts, updates sprint state if needed, and returns one integrated report.

## Package Template

Each worker package should contain:

- objective
- allowed files or directories
- out-of-scope items
- required validation
- expected return format

Each QA package should contain:

- implementation slice under test
- files or directories in scope
- explicit acceptance criteria
- commands or checks to run
- expected evidence to return

Example package skeleton:

```text
Objective: Wire baseline-controller selection into demo-client and harness config.
Allowed files: crates/demo-client/**, crates/harness/src/config.rs, configs/harness/**
Out of scope: AMC controller logic, VPS runner changes, metrics redesign.
Validation: cargo check -p demo-client && cargo check -p harness
Return: changed files, validation result, remaining integration notes.
```

## Synchronization Rules

- The master should not give the same file to multiple workers unless the overlap is purely read-only.
- Shared schema changes should be staged behind one owner at a time.
- The master should insert an integration checkpoint after any worker changes that affect config schema, output schema, or shared interfaces.
- If one worker changes a contract another worker depends on, the master must either sequence the work or issue a follow-up worker package.
- The master should run QA only after the relevant worker wave has stopped and the candidate slice is stable enough to validate.
- QA packages should stay scoped to the implementation slice they validate; they are not full regression passes unless explicitly requested.

## Recommended Sprint 1 Mapping

Parallel Wave A:

- one worker on VPS architecture
- one worker on baseline-controller integration
- one worker on analysis and metrics
- one worker on media and semantics
- one worker on docs and reporting

Wave A QA:

- one or more QA agents validating completed Wave A slices after all Wave A workers stop

Wave B after scaffolding lands:

- one worker on the AMC controller path

Wave B QA:

- one QA agent validating AMC-controller-path acceptance criteria after the Wave B worker stops

## Entry Point

Preferred entry point for manual use:

- `.github/prompts/run-sprint-master.prompt.md`

Direct agent use is also valid when a user wants to pick `Sprint Master` from the agent selector.