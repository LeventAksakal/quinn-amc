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