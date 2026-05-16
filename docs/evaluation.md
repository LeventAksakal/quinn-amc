# Evaluation

## Primary evaluation question

Does a semantic-aware QUIC transport policy improve useful multimedia delivery under constrained networks while remaining acceptably fair to standard Quinn congestion controllers?

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

Run the full workload × controller × preset matrix cleanly before adding fairness or coexistence cases.

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