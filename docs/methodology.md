# Methodology

## Objective

Evaluate whether a semantic-aware multimedia transport policy for QUIC can improve application-level outcomes relative to standard congestion controllers while preserving acceptable fairness and transport stability.

## Working hypothesis

If the sender uses application semantics such as deadlines, importance, dependency depth, and freshness, then it should be able to make better congestion-response decisions for multimedia traffic than a transport-only controller tuned for generic bulk transfer.

## Evaluation targets

The project compares at least four controller configurations:

- Quinn NewReno baseline
- Quinn Cubic baseline
- Quinn BBR baseline
- AMC controller

The comparison should use the same application workload definitions, network conditions, and measurement pipeline.

## Workload model

### VOD

VOD should model buffered delivery where late arrival is often acceptable as long as sustained throughput is high enough to avoid rebuffering.

Suggested workload properties:

- chunked object or frame-group delivery over reliable streams
- burst structure resembling encoded video segments or GOP-level variation
- receiver-side playout buffer model
- metrics that emphasize continuity rather than strict per-frame deadlines

### Live

Live traffic should model freshness-sensitive delivery where old data loses value quickly.

Suggested workload properties:

- timestamped frames generated at fixed cadence
- per-frame or per-chunk deadlines
- reliable QUIC stream transport for the main claim
- accounting for expired or stale payloads as misses rather than useful delivery

## Network scenario matrix

Use a compact, repeatable scenario matrix first. Expand only if the initial results show meaningful separation.

Initial matrix:

- RTT: 20 ms, 80 ms, 150 ms
- loss rate: 0%, 0.5%, 2%
- bottleneck bandwidth: 10 Mbps, 50 Mbps
- queueing regime: fixed bottleneck queue or emulator default

Each experiment cell should be repeated with fixed seeds where applicable.

## Metrics

Collect both transport-level and application-level metrics.

### Transport-level metrics

- goodput
- RTT summary statistics
- jitter estimate
- retransmission or recovery-related counters, where exposed
- connection completion or failure outcome

### Application-level metrics

- VOD rebuffer ratio or stall count
- VOD startup delay
- live deadline miss rate
- live on-time delivered bytes
- late-but-useless delivery ratio
- decoded frame drop rate
- delivered freshness score, if defined by the AMC policy

## Fairness and validity constraints

- Baseline and AMC runs must use the same workload traces and scenario definitions.
- Any application policy that changes reliability mode or packetization must be documented clearly because it affects comparability.
- QUIC datagrams are out of scope for the primary claim and should only be added as a secondary experiment axis if time permits.
- Mixed-workload experiments should be separate from single-class experiments because QUIC congestion control is connection-wide.

## Primary benchmark scope

Primary controller comparisons:

- Quinn NewReno
- Quinn Cubic
- Quinn BBR
- AMC semantic-aware policy

Primary benchmark questions:

- Does AMC improve multimedia utility under constrained networks?
- Does AMC remain acceptably fair when competing with Quinn baseline controllers?
- Which semantic inputs contribute most to any measured gain?

Use ablations where possible:

- transport-only baseline shell
- deadlines only
- deadlines plus importance
- deadlines plus importance plus dependency depth

## Experimental implementation plan

1. Establish a minimal Quinn client-server path.
2. Add scenario configuration files for bandwidth, RTT, and loss.
3. Implement trace-driven VOD and live generators.
4. Add baseline controller selection and result export.
5. Implement AMC controller and compare under the same matrix.
6. Produce processed tables and figures for the report.

## Artifact organization

Recommended result split:

- `results/raw/` for per-run artifacts and machine-readable outputs
- `results/processed/` for aggregated CSV or JSON summaries
- `results/figures/` for plots used in the report

## Reporting structure

The report should follow a compact systems-paper structure:

1. Introduction and problem statement
2. Background on QUIC and Quinn congestion-control integration
3. AMC design
4. Implementation details
5. Experimental setup
6. Results and discussion
7. Limitations and threats to validity
8. Conclusion and future work