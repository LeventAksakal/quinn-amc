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

## Main outcomes

### Fairness and coexistence

- Jain fairness index
- share of bottleneck goodput under competition
- RTT inflation relative to baselines

### Multimedia resilience

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

Start with a small matrix:

- RTT: 20 ms, 80 ms, 150 ms
- loss: 0%, 0.5%, 2%
- bandwidth: 10 Mbps, 50 Mbps

Preferred path-control mechanism:

- Linux `tc netem` for delay and loss
- `tbf` or `htb` for bandwidth shaping when needed

Then add competition cases:

- AMC vs NewReno
- AMC vs Cubic
- AMC vs BBR

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