# Final Report

## Scope And Claim Boundary

This repository is complete at the AMC v1 boundary, not at a broader AMC v2 or BBR-replacement boundary.

The claim supported by the frozen evidence is narrow:

- AMC v1 improves the hardest constrained live cells relative to `new_reno` and `cubic`.
- BBR remains the strongest overall baseline on freshness-sensitive live metrics across the canonical fixed matrix.
- VOD is included as required supporting evidence, but AMC v1 is not a startup-delay winner.
- Fairness is required and is interpreted at the throughput-sharing level against BBR, not as proof of application-quality parity under competition.

## Experiment Structure

The final evidence comes from exactly two canonical VPS suites:

- `configs/harness/vps_fixed_preset_controller_matrix.json`
- `configs/harness/vps_host_live_coexistence_bbr_guardrail.json`

The workflow is intentionally split because the current docker runner remains single-client only:

- the fixed matrix runs through `scripts/experiments/run_linux_vps_suite.sh` with host-managed `tc` on the demo-server container host-veth
- the fairness guardrail runs directly through the host `harness` binary with `tc` on `lo`

That split is part of the frozen methodology rather than an unresolved implementation accident.

## Frozen Evidence Inventory

The canonical processed artifacts are:

- [matrix summary](../results/processed/harness/vps_fixed_preset_controller_matrix_summary.json)
- [matrix comparison export](../results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json)
- [fairness summary](../results/processed/harness/vps_host_live_coexistence_bbr_guardrail_summary.json)
- [fairness comparison export](../results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json)

The fixed matrix provides `10 / 10` complete matrix groups. The fairness guardrail provides `2 / 2` complete matrix groups.

The canonical Phase 6 figure set contains 39 suite-prefixed SVGs under `results/figures/harness/`. Representative figures for the main claims are:

- [matrix live useful media ratio](../results/figures/harness/vps_fixed_preset_controller_matrix_live_realtime_useful_media_ratio.svg)
- [matrix live deadline miss rate](../results/figures/harness/vps_fixed_preset_controller_matrix_live_realtime_deadline_miss_rate.svg)
- [matrix live jitter](../results/figures/harness/vps_fixed_preset_controller_matrix_live_realtime_average_jitter_ms.svg)
- [matrix VOD startup delay](../results/figures/harness/vps_fixed_preset_controller_matrix_vod_realtime_vod_startup_delay_ms.svg)
- [fairness throughput share](../results/figures/harness/vps_host_live_coexistence_bbr_guardrail_live_realtime_fairness_foreground_throughput_share.svg)
- [fairness Jain index](../results/figures/harness/vps_host_live_coexistence_bbr_guardrail_live_realtime_fairness_jain_index.svg)

## Results

### Live Single-Flow Matrix

`wifi_unstable` is the cleanest bounded success case for AMC v1.

- AMC matches BBR on useful-media ratio and deadline-miss rate.
- AMC is materially better than `cubic` and `new_reno` on those same live-quality metrics.
- BBR still wins clearly on latency distribution and jitter, so this is not a broad BBR-parity claim.

`lte_constrained` remains the hardest live cell.

- AMC improves over `cubic` and `new_reno` on useful delivery and deadline misses.
- AMC still trails BBR by a meaningful margin on age of information, latency, and jitter.
- The correct headline is therefore constrained-live improvement over the loss-based baselines, not broad superiority across the matrix.

The other fixed presets remain neutral or bounded for AMC v1. They do not justify widening the claim beyond the constrained-live cases.

### VOD Supporting Evidence

VOD continuity stays stable across the frozen matrix, but AMC v1 remains slower to start playback than every baseline on the hardest constrained preset.

- On `wifi_unstable`, startup delay is `2305 ms` for AMC versus `2006 ms` for BBR.
- On `lte_constrained`, startup delay is `3509 ms` for AMC versus `2023 ms` for BBR.
- All four controllers retain useful ratio `1.0` and rebuffer ratio `0.0` on those constrained VOD presets.

The correct VOD interpretation is therefore limited: AMC v1 preserves continuity, but it should not be described as VOD-competitive on startup.

### Fairness Guardrail

The coexistence guardrail validates fairness against BBR at the throughput-sharing level.

- Foreground throughput share remains near `0.5` across the required guardrail suite.
- Jain fairness remains effectively `1.0`.
- AMC does not gain its bounded live improvements by gaming bottleneck share.

The fairness claim stays narrow. Throughput fairness against BBR does not imply freshness parity with BBR under competition on the hardest constrained live path.

## Reproducibility

Canonical VPS rerun commands:

```bash
cd /home/leven/quinn-amc
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_fixed_preset_controller_matrix.json
source "$HOME/.cargo/env"
cargo build -p harness
sudo ./target/debug/harness run-suite --config configs/harness/vps_host_live_coexistence_bbr_guardrail.json
sudo chown -R "$USER":"$USER" results
```

Canonical figure regeneration commands:

```powershell
cargo run -p harness -- plot-suite --comparison results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --output-dir results/figures/harness
cargo run -p harness -- plot-suite --comparison results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --output-dir results/figures/harness
```

Canonical report-package command:

```powershell
cargo run -p harness -- package-report --report docs/final-report.md --matrix-comparison results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --fairness-comparison results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --figure-dir results/figures/harness --output-dir results/reports/final
```

The local workspace freezes processed outputs and figures. It does not include the full copied VPS raw-report set for every canonical cell, so raw-report provenance remains anchored by the processed artifacts and the validated VPS handoff.

## Limitations

- The repository completion claim stops at AMC v1. It does not include AMC v2 state expansion.
- BBR remains the strongest overall live baseline in the frozen matrix.
- The canonical fairness workflow still uses a host-run topology exception because the docker VPS runner remains single-flow only.
- The report package is Markdown-first in this phase. HTML or PDF export remains outside scope.

## Future Work Outside Scope

- AMC v2 controller-state expansion
- broader coexistence topologies or docker-runner coexistence parity
- improved live tail-latency behavior under the hardest constrained presets
- reduced VOD startup delay for AMC
- alternative export formats for the report package