# Evaluation

## Primary evaluation question

Does a semantic-aware QUIC transport policy improve useful multimedia delivery under constrained networks while remaining acceptably fair to standard Quinn congestion controllers?

## Phase 1 Frozen Evaluation Boundary

- Repository completion is bounded to AMC v1 under the current latest-sample, connection-wide runtime utility design.
- Live traffic is the primary claim surface.
- VOD remains required supporting evidence and must be reported honestly even where AMC is not competitive on startup.
- Final reported evidence requires both the fixed-preset VPS matrix and the separate fairness guardrail suite.
- Local parity runs remain required validation support, but they do not replace the VPS evidence path.
- Dynamic adaptation suites, QUIC datagrams in the main claim, AMC v2 expansion, and broader comparative demo work remain out of scope for completion.

## Baselines

- Quinn NewReno
- Quinn Cubic
- Quinn BBR
- AMC

## Traffic classes

- VOD over QUIC streams
- live over QUIC streams

## Locked benchmark shape

The main benchmark uses a fixed matrix:

- workload: `vod`, `live`
- controller: `new_reno`, `cubic`, `bbr`, `amc_preview`
- network preset: fixed named `tc` profiles

The network preset labels are convenience names only. Every preset must still encode explicit RTT, jitter, bandwidth, loss, and queue settings.

Dynamic adaptation scenarios are not part of the primary benchmark path.

## Main outcomes

### Fairness and coexistence

- Jain fairness index
- share of bottleneck goodput under competition
- RTT inflation relative to baselines

Fairness and coexistence are mandatory final evidence, but they remain a separate guardrail family rather than part of the single-flow matrix.

### Multimedia resilience

- average age of information for live traffic
- stall count
- rebuffer ratio
- deadline miss rate
- decoded frame drop rate
- late-but-useless delivery ratio

### Transport behavior

- connection goodput
- loss and recovery counters where available
- completion success or failure

## Scenario plan

Use named fixed presets rather than raw cartesian sweeps in the primary path.

Initial preset family:

- `wired_clean`
- `wifi_moderate`
- `wifi_unstable`
- `lte_moderate`
- `lte_constrained`

Each preset should be represented in config by fixed `tc netem` and rate-limit parameters.

Preferred path-control mechanism:

- Linux `tc netem` for delay and loss
- `tbf` or `htb` for bandwidth shaping when needed

The fixed-preset single-flow matrix and the separate fairness guardrail suite are both required final evidence families.

Frozen final-evidence configs:

- `configs/harness/vps_fixed_preset_controller_matrix.json`
- `configs/harness/vps_host_live_coexistence_bbr_guardrail.json`

Required local parity configs:

- `configs/harness/local_controller_matrix.json`
- `configs/harness/local_live_immediate_amc_bbr_coexistence.json`

## Recommended ablations

- utility disabled
- deadlines only
- deadlines plus importance
- deadlines plus importance plus dependency depth

## Workload artifact path

The first benchmarkable workload path should be:

1. open media asset in `data/raw/`
2. offline `ffmpeg` segmentation and `ffprobe` manifest extraction
3. Quinn client timed replay over streams
4. receiver logging of arrival time, lateness, and usefulness

## Secondary work if time permits

- TCP coexistence studies
- datagram-based live experiments
- larger scenario sweeps or burst-loss regimes
- AMC v2 sender-state expansion
- legacy docker-runner coexistence parity with the host-run fairness path

## Phase 4 Controller-Completion Criteria

Phase 4 freezes repository completion at AMC v1 rather than reopening controller design.

AMC is considered complete for this repository when:

- controller identity and provenance remain auditable from config through processed outputs
- the frozen live matrix shows bounded semantic-aware value on the harder constrained presets
- the required BBR guardrail shows acceptable throughput fairness and Jain fairness
- VOD is reported honestly as supporting evidence rather than as an AMC win condition
- no AMC v2 state expansion is required to defend the repository claim

This phase does not require AMC to beat BBR on every cell. It requires a bounded, evidence-backed AMC v1 claim that later phases can freeze without reopening controller scope.

## Current AMC v1 Interpretation

The current frozen VPS evidence supports a narrow live-primary claim:

- `amc_preview` improves the hardest constrained live cells relative to `new_reno` and `cubic`, especially on deadline-miss rate and useful utility under `wifi_unstable` and `lte_constrained`
- `bbr` remains the strongest overall baseline on live freshness-sensitive outcomes across the matrix
- VOD throughput and aggregate utility are similar across controllers, but AMC v1 does not win on startup delay and is materially worse than BBR on `vod/lte_constrained`
- the required fairness guardrail shows near-even throughput shares and near-perfect Jain fairness for AMC v1 against BBR, so fairness is acceptable at the throughput-sharing level even though AMC freshness still trails BBR in the same family

The phase output is therefore a bounded AMC v1 claim, not a claim of broad BBR replacement.

See [docs/amc-milestone.md](docs/amc-milestone.md) for the compact evidence readout and the explicitly deferred AMC v2 follow-up work.

## Phase 5 Frozen Result Families

Phase 5 freezes the result base around four canonical processed artifacts:

- `results/processed/harness/vps_fixed_preset_controller_matrix_summary.json`
- `results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json`
- `results/processed/harness/vps_host_live_coexistence_bbr_guardrail_summary.json`
- `results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json`

Those files provide complete coverage for the current final evidence families: `10 / 10` complete fixed-matrix groups and `2 / 2` complete fairness-guardrail groups.

### Live final-evidence interpretation

- `wifi_unstable` remains the clearest bounded AMC win cell relative to `new_reno` and `cubic`: AMC matches BBR on deadline-miss rate and useful-media ratio while staying materially better than the loss-based baselines on those same metrics.
- `lte_constrained` remains the hardest cell overall: AMC improves on `new_reno` and `cubic` for useful delivery and deadline misses, but still trails BBR by a meaningful margin on age of information, tail latency, and overall useful utility.
- `wired_clean`, `wifi_moderate`, and `lte_moderate` remain neutral or bounded cells for AMC v1: they do not justify a broad claim over BBR, but they remain consistent with the narrower Phase 4 interpretation.

### VOD final-evidence interpretation

- VOD useful delivery and rebuffer behavior remain stable across the frozen matrix.
- Startup delay remains the main VOD deficit for AMC v1, especially on `lte_constrained` where AMC is slower than every baseline and far behind BBR.
- VOD therefore remains a bounded supporting evidence family rather than a headline AMC success case.

### Fairness final-evidence interpretation

- The fairness guardrail suite confirms that AMC is not gaming throughput share against BBR.
- The guardrail still shows that fairness at the throughput-sharing level does not imply freshness parity with BBR on the hardest constrained live path.
- The fairness claim is therefore narrow and specific: AMC v1 is acceptable under the required BBR guardrail, not universally equivalent to BBR under competition.

See [docs/evidence-freeze.md](docs/evidence-freeze.md) for the exact artifact inventory and include/exclude rules that Phase 6 and Phase 7 must follow.