# Evidence Freeze

## Purpose

This note records the Phase 5 evidence-freeze boundary for the repository.

Its job is to tell later phases exactly which artifacts count as the frozen evidence base, which outputs remain support-only, and which commands and paths are authoritative for reproduction.

## Frozen Final-Evidence Set

Phase 5 freezes the benchmark evidence set around the two canonical VPS suites already selected by Phase 1 and Phase 2.

### Canonical processed artifacts in this workspace

- `results/processed/harness/vps_fixed_preset_controller_matrix_summary.json`
- `results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json`
- `results/processed/harness/vps_host_live_coexistence_bbr_guardrail_summary.json`
- `results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json`

These four files are the authoritative local inputs for Phase 6 figures and Phase 7 report packaging.

### Canonical suite coverage

- The fixed-preset matrix summary contains `40` runs.
- The fixed-preset comparison export contains `10 / 10` complete matrix groups.
- The fairness guardrail summary contains `8` foreground runs.
- The fairness guardrail comparison export contains `2 / 2` complete matrix groups.

### Raw artifact boundary

The processed artifacts above reference canonical raw report paths such as `results/raw/harness/vod_realtime_new_reno_wired_clean_report.json` and `results/raw/harness/live_realtime_amc_preview_lte_constrained_with_bbr_report.json`.

The current local workspace contains the frozen processed outputs and the handoff documentation for the VPS reruns, but it does not contain the full copied VPS raw-report set for every canonical cell.

For Phase 5, the authoritative raw-report source remains:

- the `report_path` fields embedded in the frozen processed outputs
- the remote VPS processed-results handoff documented in [docs/vps-results-handoff.md](docs/vps-results-handoff.md)
- the remote source location `/home/leven/quinn-amc/results/processed/harness/` recorded in that handoff note

That means the repository freeze point is honest about what is local and what remains anchored by the validated VPS handoff.

## Included And Excluded Result Families

### Included as frozen final evidence

- `configs/harness/vps_fixed_preset_controller_matrix.json`
- `configs/harness/vps_host_live_coexistence_bbr_guardrail.json`
- the four canonical processed artifacts listed above

### Included as regression or reproducibility support only

- `configs/harness/local_controller_matrix.json`
- `configs/harness/local_live_immediate_amc_bbr_coexistence.json`
- `results/processed/harness/local_controller_matrix_summary.json`
- `results/processed/harness/local_controller_matrix_comparison.json`
- `results/processed/harness/local_live_immediate_amc_bbr_coexistence_summary.json`
- `results/processed/harness/local_live_immediate_amc_bbr_coexistence_comparison.json`

These local artifacts remain important for regression coverage and reproducibility support, but they are not part of the final evidence claim.

### Excluded from the final evidence set

- workflow-validation suites such as `configs/harness/vps_baseline_vod_live.json` and `configs/harness/vps_demo_vod_live.json`
- exploratory or non-canonical configs such as `configs/harness/vps_live_realtime_controller_matrix.json`, `configs/harness/vps_lte_constrained_live_matrix.json`, and `configs/harness/vps_live_coexistence_bbr_guardrail.json`
- local focused smoke outputs such as `local_live_immediate_amc_preview_*`, `local_live_immediate_bbr_only_*`, and `demo_vod_live_summary.json`

Phase 6 and Phase 7 should not consume excluded outputs when building figures or report text.

## Reproduction Contract

### Canonical VPS fixed matrix

```bash
cd /home/leven/quinn-amc
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_fixed_preset_controller_matrix.json
```

### Canonical VPS fairness guardrail

```bash
cd /home/leven/quinn-amc
source "$HOME/.cargo/env"
cargo build -p harness
sudo ./target/debug/harness run-suite --config configs/harness/vps_host_live_coexistence_bbr_guardrail.json
sudo chown -R "$USER":"$USER" results
```

These are the only reproduction commands Phase 5 freezes as part of the final evidence contract.

## Phase 6 Figure Contract

### Canonical figure commands

```powershell
cargo run -p harness -- plot-suite --comparison results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --output-dir results/figures/harness
cargo run -p harness -- plot-suite --comparison results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --output-dir results/figures/harness
```

### Validated Phase 6 figure outputs

- The validated final figure directory contains `39` suite-prefixed SVGs under `results/figures/harness/`.
- Fixed-matrix figure names follow `vps_fixed_preset_controller_matrix_overview_<metric>.svg` or `vps_fixed_preset_controller_matrix_<mode>_<pace>_<metric>.svg`.
- Fairness-guardrail figure names follow `vps_host_live_coexistence_bbr_guardrail_overview_<metric>.svg` or `vps_host_live_coexistence_bbr_guardrail_<mode>_<pace>_<metric>.svg`.
- The fixed matrix contributes usefulness, deadline miss rate, throughput, delivery latency, jitter, live age of information, VOD startup delay, and VOD rebuffer ratio figures.
- The fairness guardrail contributes the live transport and usefulness figures plus foreground throughput share, fairness throughput ratio, and Jain fairness index.

## Frozen Interpretation Summary

### Live single-flow matrix

- AMC v1 shows bounded value on the hardest constrained live presets, especially `wifi_unstable` and `lte_constrained`, relative to `new_reno` and `cubic`.
- BBR remains the strongest overall baseline on freshness-sensitive live metrics across the fixed matrix.
- The live-primary claim therefore stays bounded to constrained-live improvement over the loss-based baselines, not broad superiority over BBR.

### VOD supporting evidence

- VOD continuity remains stable across controllers on the constrained presets.
- AMC v1 does not win on startup delay and is materially worse than BBR on the hardest constrained VOD preset.
- VOD is therefore part of the frozen evidence package as supporting boundedness evidence, not as an AMC headline win condition.

### Fairness guardrail

- The BBR guardrail remains acceptable at the throughput-sharing level for AMC v1.
- Jain fairness stays effectively perfect across the required fairness suite.
- The fairness claim is therefore that AMC v1 is throughput-fair against BBR under the required guardrail, not that AMC matches BBR on application freshness while competing.

## Rules For Later Phases

- Phase 6 should generate figures only from the canonical processed artifacts in this note unless a later phase explicitly reopens the evidence boundary.
- Later figure reruns should use the canonical figure commands above and keep the suite-prefixed naming convention intact.
- Phase 7 should build the report package around the same canonical artifacts and the same bounded claim.
- Phase 8 should choose a showcase raw report from the frozen evidence family, not from workflow-validation or exploratory outputs.
- Phase 9 should validate the final artifact set against this freeze note instead of re-deciding what counts as evidence.