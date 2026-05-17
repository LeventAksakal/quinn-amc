# Live Demo

## Purpose

The final live demo is a single-run ratatui introspection viewer for one frozen AMC v1 report.

Its job is not to compare controllers side by side. Its job is to let a reviewer watch one canonical constrained-live AMC run and inspect the workload-facing behavior, runtime utility evolution, and delivery outcomes that underpin the bounded AMC claim.

## Canonical Showcase Run

The showcase raw report is:

- `results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json`

This run comes from the frozen fixed-preset VPS matrix and corresponds to the hardest constrained live preset discussed in the final report.

Why this run was chosen:

- it belongs to a frozen evidence family rather than a local support-only run
- it is the hardest constrained live AMC case, so the utility signal is visible under real pressure
- it stays honest about the repository claim: AMC improves on `new_reno` and `cubic`, but BBR remains stronger overall on freshness-sensitive metrics

If the raw report is missing locally, retrieve exactly this file from the VPS rather than swapping in a support-only local report:

```powershell
gcloud compute scp quinn-amc-vps:/home/leven/quinn-amc/results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json --zone europe-west6-c
```

## Launch

```powershell
cargo run -p harness -- live-demo --report results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json --speed 1.0
```

Controls:

- `q` or `Esc` exits the demo

## What The Demo Shows

### Replay Progress

- overall progress through the media observations in the showcase run

### Transfer Summary

- asset name
- controller identity
- mode
- useful versus late media counts
- payload size
- runtime utility sample count and observed utility range

### Showcase Status

- whether the loaded report matches the frozen canonical showcase run
- current replay step and elapsed replay time
- current usefulness status
- deadline status for the active observation
- delivery latency and age of information

### Current Observation

- segment sequence and kind
- start time, duration, and deadline
- client send and server receive timestamps
- payload size
- lateness in milliseconds
- segment path

### Runtime Utility Telemetry

This panel exposes only telemetry that is already stored in the raw report:

- traffic class
- importance
- dependency depth
- dependency ready state
- queue delay
- estimated RTT
- observed utility score
- smoothed utility score
- EWMA weight
- ack gain
- loss reduction factor

### AMC Controller Snapshot

When the raw report was generated with the current telemetry schema, the demo also shows the AMC controller snapshot persisted by the sender-side congestion controller bridge:

- controller phase
- last controller event
- congestion window
- slow-start threshold
- growth step
- current MTU
- initial, minimum, configured maximum, and class maximum windows

### Trend Panels

- utility score over time
- congestion window over time
- positive deadline miss over time

## Interpretation Notes

- `useful` means the segment still had value when it arrived at the receiver.
- A positive `lateness_ms` means the segment missed its deadline.
- `ack_gain` and `loss_reduction_factor` are AMC runtime telemetry values already recorded in the raw report.
- The controller snapshot is report-backed, not inferred from post hoc analysis. Older raw reports may lack it even if they already contain the AMC signal fields.
- The viewer is single-run only by design. It is not intended to become a comparative multi-controller lab.

## Scope Boundary

- The demo is frozen to a canonical raw report from the final evidence family.
- Workflow-validation, exploratory, and local support-only reports are outside the final Phase 8 artifact unless a later phase explicitly reopens that boundary.
- The demo visualizes report-backed telemetry only. It does not claim access to AMC internals that were not persisted in the raw artifact.