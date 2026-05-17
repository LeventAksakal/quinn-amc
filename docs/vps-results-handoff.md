# VPS Results Handoff

Date: 2026-05-17

## Scope

This handoff covers the AMC traffic-class update, harness coexistence support, and the latest VPS reruns completed after fixing two validity bugs discovered during execution.

## What Was Validated

- Local validation passed for `cargo test -p amc-core` and `cargo check -p amc-core -p demo-client -p demo-server -p harness`.
- Local coexistence smoke test passed with `configs/harness/local_live_immediate_amc_bbr_coexistence.json`.
- The fixed preset VPS matrix completed successfully with `configs/harness/vps_fixed_preset_controller_matrix.json`.
- The coexistence fairness suite completed successfully only through the host-run harness path with `configs/harness/vps_host_live_coexistence_bbr_guardrail.json`.

## Critical VPS Fixes Discovered During Rerun

1. The standalone demo-server could close its endpoint before the client finished reading the summary response.
   Fix: `crates/demo-server/src/lib.rs` now waits on `send.stopped().await` after `send.finish()`.

2. The VPS compose path was not propagating `--controller` to demo-client, so every supposed matrix cell silently ran with the demo-client default controller `cubic`.
   Fix: the compose client command must include `--controller ${DEMO_CLIENT_CONTROLLER:-cubic}`.

These fixes were necessary for the rerun results below to be methodologically valid.

## Result Artifacts

Primary processed artifacts copied into the local workspace:

- `results/processed/harness/vps_fixed_preset_controller_matrix_summary.json`
- `results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json`
- `results/processed/harness/vps_host_live_coexistence_bbr_guardrail_summary.json`
- `results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json`

Remote source location on the VPS:

- `/home/leven/quinn-amc/results/processed/harness/`

## Fixed Matrix Readout

### Live, `wifi_unstable`

- `amc_preview`: throughput `0.3416339155039516` Mbps, useful ratio `1.0`, deadline miss rate `0.0`, p95 latency `451 ms`, average jitter `97.56 ms`
- `bbr`: throughput `0.34170059529618424` Mbps, useful ratio `1.0`, deadline miss rate `0.0`, p95 latency `92 ms`, average jitter `25.76 ms`
- `cubic`: throughput `0.34154227327041725` Mbps, useful ratio `0.9761904761904762`, deadline miss rate `0.023809523809523808`, p95 latency `644 ms`, average jitter `147 ms`
- `new_reno`: throughput `0.3416839229080263` Mbps, useful ratio `0.9761904761904762`, deadline miss rate `0.023809523809523808`, p95 latency `767 ms`, average jitter `177.66 ms`

Interpretation: AMC now matches BBR on usefulness and miss rate on unstable WiFi, and clearly beats Cubic and NewReno on quality, but BBR still wins decisively on tail latency and jitter.

### Live, `lte_constrained`

- `amc_preview`: throughput `0.3393164066285493` Mbps, useful ratio `0.9523809523809523`, deadline miss rate `0.047619047619047616`, p95 latency `994 ms`, average jitter `228.32 ms`
- `bbr`: throughput `0.34169225889872895` Mbps, useful ratio `1.0`, deadline miss rate `0.0`, p95 latency `103 ms`, average jitter `24.78 ms`
- `cubic`: throughput `0.3415839227354763` Mbps, useful ratio `0.9047619047619048`, deadline miss rate `0.09523809523809523`, p95 latency `1477 ms`, average jitter `305.20 ms`
- `new_reno`: throughput `0.3428381474591208` Mbps, useful ratio `0.9047619047619048`, deadline miss rate `0.09523809523809523`, p95 latency `1711 ms`, average jitter `264.90 ms`

Interpretation: AMC improved over Cubic and NewReno on the hardest live path but remains materially behind BBR, especially on latency distribution.

### VOD continuity, constrained presets

`wifi_unstable` startup delay:

- `bbr`: `2006 ms`
- `cubic`: `2096 ms`
- `new_reno`: `2204 ms`
- `amc_preview`: `2305 ms`

`lte_constrained` startup delay:

- `bbr`: `2023 ms`
- `cubic`: `2192 ms`
- `new_reno`: `2791 ms`
- `amc_preview`: `3509 ms`

All four controllers had useful ratio `1.0` and rebuffer ratio `0.0` on these VOD constrained presets.

Interpretation: the traffic-class-aware AMC tuning kept VOD continuity stable, but AMC is still the slowest to start and should not be considered VOD-competitive yet.

## Coexistence Guardrail Readout

Validated suite: `configs/harness/vps_host_live_coexistence_bbr_guardrail.json`

### `wifi_unstable` with concurrent BBR competitor

- `amc_preview`: foreground throughput `0.341967574958492` Mbps, useful ratio `1.0`, miss rate `0.0`, competitor throughput `0.3417089321004221` Mbps, foreground share `0.5001891558766459`, Jain `0.9999998568802381`
- `bbr`: foreground share `0.49959622161315587`, Jain `0.9999993478524826`
- `cubic`: foreground share `0.5002074385005857`, Jain `0.9999998278771035`
- `new_reno`: foreground share `0.4999939022159347`, Jain `0.9999999998512681`

### `lte_constrained` with concurrent BBR competitor

- `amc_preview`: foreground throughput `0.33950558747242626` Mbps, useful ratio `0.9285714285714286`, miss rate `0.07142857142857142`, competitor throughput `0.34154227327041725` Mbps, foreground share `0.4985047410649161`, Jain `0.9999910568828482`
- `bbr`: foreground share `0.5001280409731114`, Jain `0.9999999344220414`
- `cubic`: foreground share `0.5000731939395417`, Jain `0.9999999785705893`
- `new_reno`: foreground share `0.49991466744685`, Jain `0.9999999708734225`

Interpretation: AMC is not gaming throughput share. Fairness stayed essentially perfect against BBR, but AMC still loses application quality on the hardest constrained live path while being fair.

## Important Methodology Constraint

`configs/harness/vps_live_coexistence_bbr_guardrail.json` is not enough by itself for the legacy VPS docker runner. `scripts/experiments/run_linux_vps_suite.sh` still launches exactly one foreground demo-client container per run, so it cannot produce coexistence raw reports.

The validated coexistence measurements therefore came from running the harness directly on the VPS host with `tc` applied to `lo`, not from the container host-veth runner path.

## Recommended Next Work

1. Extend `scripts/experiments/run_linux_vps_suite.sh` so coexistence runs launch both foreground and competitor clients and preserve the existing host-veth shaping path.
2. Improve AMC live tail latency under `wifi_unstable` and `lte_constrained` without sacrificing the fairness result already achieved.
3. Reduce AMC VOD startup delay; this remains the clearest VOD deficit after the latest tuning.

## Reproduction Commands

Fixed matrix on VPS:

```bash
cd /home/leven/quinn-amc
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_fixed_preset_controller_matrix.json
```

Validated coexistence guardrail on VPS host:

```bash
cd /home/leven/quinn-amc
source "$HOME/.cargo/env"
cargo build -p harness
sudo ./target/debug/harness run-suite --config configs/harness/vps_host_live_coexistence_bbr_guardrail.json
sudo chown -R leven:leven results
```