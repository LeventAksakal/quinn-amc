# Methodology

## Objective

Evaluate whether sender-visible multimedia semantics can improve application-level QUIC outcomes relative to standard congestion controllers while preserving acceptable fairness and transport stability.

## Scope Boundary

Repository completion is frozen at the AMC v1 boundary.

- The sender scores media units from `traffic class`, `deadline`, `importance`, `dependency depth`, `freshness window`, and size.
- The congestion controller consumes only the latest connection-wide utility sample rather than a richer AMC v2 state model.
- Live traffic is the primary claim surface.
- VOD remains required supporting evidence and must be reported honestly where AMC is weaker.
- Final evidence requires both the VPS fixed matrix and the VPS fairness guardrail.
- AMC v2 expansion, docker-runner coexistence parity, dynamic adaptation suites, and QUIC datagrams in the main claim remain out of scope.

## System Model

The workspace is a Cargo multi-crate research artifact:

- `crates/amc-core`: application-semantics and congestion-policy logic
- `crates/demo-client`: replay sender
- `crates/demo-server`: receiver and report sink
- `crates/harness`: suite execution, analysis, plotting, packaging, and live demo

The design stays trace-driven. Media is preprocessed offline with `ffmpeg` and `ffprobe` into replay manifests and segmented assets. Runtime experiments consume those prebuilt artifacts rather than a full media stack.

The semantic interface is codec-agnostic. The main sender-visible inputs are `deadline`, `importance`, `dependency depth`, and `freshness window`, with replay-manifest hints preferred over harness fallback defaults.

## Workloads And Matrix

The benchmark surface is fixed and preset-driven.

- workloads: `vod`, `live`
- controllers: `new_reno`, `cubic`, `bbr`, `amc_preview`
- presets: `wired_clean`, `wifi_moderate`, `wifi_unstable`, `lte_moderate`, `lte_constrained`

Each preset must map to explicit shaping parameters for RTT, jitter, bandwidth, loss, and queue behavior. The main comparison uses one fixed condition per matrix cell rather than dynamic within-run adaptation.

### VOD

VOD models buffered replay over reliable streams. It is part of the final artifact because continuity and startup behavior are important, but it is not the primary AMC success surface.

### Live

Live models freshness-sensitive replay over reliable streams with tight deadlines and low lookahead. This is the primary evaluation surface for the AMC v1 claim.

## Metrics

The reporting model keeps workload-facing metrics primary and transport-facing metrics secondary.

- live: useful-media ratio, deadline miss rate, average age of information, delivery latency, jitter, throughput
- VOD: startup delay, rebuffer count, rebuffer duration, rebuffer ratio, useful-media ratio, throughput
- fairness: foreground throughput share, throughput ratio, Jain fairness index
- transport support: RTT and completion outcome where exposed

## Canonical Execution Model

The validated evidence path is split across two execution modes on one GCP Linux VM.

### Fixed matrix

- config: `configs/harness/vps_fixed_preset_controller_matrix.json`
- runner: `scripts/experiments/run_linux_vps_suite.sh`
- topology: demo client and server run in containers on one host; the host applies `tc` to the demo-server container host-veth
- output flow: raw reports are generated first, then analyzed into processed artifacts

### Fairness guardrail

- config: `configs/harness/vps_host_live_coexistence_bbr_guardrail.json`
- runner: host `harness run-suite`
- topology: direct host execution with `tc` on `lo`
- reason for split: the current docker runner still launches only one foreground client and cannot emit coexistence raw reports

### Local parity

The required local support suites are:

- `configs/harness/local_controller_matrix.json`
- `configs/harness/local_live_immediate_amc_bbr_coexistence.json`

These runs are for regression and reproducibility support. They are not a substitute for the canonical VPS evidence.

## Evidence Contract

The frozen final-evidence inputs are exactly four processed artifacts:

- `results/vps/processed/harness/vps_fixed_preset_controller_matrix_summary.json`
- `results/vps/processed/harness/vps_fixed_preset_controller_matrix_comparison.json`
- `results/vps/processed/harness/vps_host_live_coexistence_bbr_guardrail_summary.json`
- `results/vps/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json`

These artifacts define the bounded claim surface.

- the fixed matrix provides `10 / 10` complete matrix groups
- the fairness guardrail provides `2 / 2` complete fairness groups
- local parity outputs remain support-only
- workflow-validation and exploratory outputs remain excluded from the final claim, figures, and report package

The canonical report package is built from the two comparison exports and the figure directory `results/vps/figures/harness/`. The validated package contains `39` figures plus the four packaged processed artifacts under `results/vps/reports/final/`.

The canonical showcase raw report for the live demo is:

- `results/vps/raw/harness/live_realtime_amc_preview_lte_constrained_report.json`

## Current Artifact State

As of the latest verified VPS rerun on `2026-05-17`:

- local suites now write under `results/local/`
- VPS suites now write under `results/vps/` on the VM and should be copied back into the same path locally
- the canonical processed evidence, figures, report package, and demo artifact all belong under `results/vps/`

## Reproducibility Rules

- Regenerate replay assets with `scripts/media/preprocess_streams.sh` whenever referenced segment payloads change.
- Treat replay manifests as checked inputs; stale manifests should not be silently reused.
- Prefer processed outputs whose `input_provenance` hashes match the suite config, replay manifest, and raw report under review.
- Keep config classification explicit so final evidence, parity, workflow-validation, and exploratory suites do not drift together.
- Baseline and AMC runs must use the same workload traces and scenario definitions.
- QUIC datagrams remain out of scope for the primary claim.

Canonical commands:

```bash
cd /home/leven/quinn-amc
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_fixed_preset_controller_matrix.json
source "$HOME/.cargo/env"
cargo build -p harness
sudo ./target/debug/harness run-suite --config configs/harness/vps_host_live_coexistence_bbr_guardrail.json
sudo chown -R "$USER":"$USER" results/vps
```

```powershell
cargo run -p harness -- plot-suite --comparison results/vps/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --output-dir results/vps/figures/harness
cargo run -p harness -- plot-suite --comparison results/vps/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --output-dir results/vps/figures/harness
cargo run -p harness -- package-report --report docs/final-report.md --matrix-comparison results/vps/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --fairness-comparison results/vps/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --figure-dir results/vps/figures/harness --output-dir results/vps/reports/final
cargo run -p harness -- live-demo --report results/vps/raw/harness/live_realtime_amc_preview_lte_constrained_report.json --speed 1.0
```

If the canonical raw demo report is missing locally, retrieve that exact file from the VPS with `gcloud compute scp` rather than substituting a support-only local report.

## Interpretation And Limits

The frozen evidence supports a bounded claim:

- AMC v1 improves the hardest constrained live cells relative to `new_reno` and `cubic`
- BBR remains the strongest overall live baseline in the matrix
- VOD continuity is stable, but AMC is not competitive on startup delay
- fairness against BBR is acceptable at the throughput-sharing level

The evidence does not support these stronger claims:

- broad superiority over BBR
- AMC v2 controller-state expansion
- docker-runner coexistence parity
- a full raw-artifact mirror in this local workspace