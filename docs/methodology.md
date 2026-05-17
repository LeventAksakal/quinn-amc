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

## Phase 1 Frozen Completion Boundary

Phase 1 freezes the repository around the current AMC v1 experiment boundary.

- Completion is an AMC v1 claim only. The repository does not need an AMC v2 state expansion to count as finished.
- The current AMC claim stays bounded to a connection-wide congestion controller modulated by the latest sender utility sample.
- Live traffic is the primary claim surface. VOD remains required supporting evidence so continuity behavior and deficits stay visible in the final artifact.
- Final reported evidence comes from the VPS fixed-preset matrix and the VPS fairness guardrail suite.
- Local controller-matrix and coexistence runs remain mandatory parity coverage, but they support reproducibility and regression checking rather than replacing the VPS evidence path.
- Dynamic adaptation suites, QUIC datagrams in the main claim, AMC v2 sender-state expansion, and a comparative multi-controller demo lab stay outside the frozen completion boundary.

## Phase 2 Frozen Workflow Model

Phase 2 freezes the documentation around the workflow that is implemented today.

- The canonical VPS evidence model is split across two execution modes on one GCP Linux VM.
- The fixed-preset single-flow matrix uses `scripts/experiments/run_linux_vps_suite.sh`, host-managed `tc` on the demo-server container host-veth, and post-run `analyze-suite` through the harness container.
- The mandatory fairness guardrail uses direct host `harness run-suite` with `tc` applied to `lo`, because the legacy docker runner still launches exactly one foreground demo-client container per run.
- Local parity configs remain required validation support.
- Runner unification is deferred to Phase 3. Phase 2 documents the split explicitly rather than implying a single path that does not yet exist.

## Workload model

The implementation should stay trace-driven. Use `ffmpeg` and `ffprobe` offline to turn open source clips into replayable segment sets and timing manifests rather than embedding a full player or media pipeline in the Quinn client.

### VOD

VOD should model buffered delivery where late arrival is often acceptable as long as sustained throughput is high enough to avoid rebuffering.

VOD remains part of the final artifact, but it is a supporting evidence family rather than the primary success criterion for the AMC claim.

Suggested workload properties:

- chunked object or frame-group delivery over reliable streams
- bounded prefetch ahead of playout
- burst structure resembling encoded video segments or GOP-level variation
- receiver-side playout buffer model
- metrics that emphasize continuity rather than strict per-frame deadlines

### Live

Live traffic should model freshness-sensitive delivery where old data loses value quickly.

Live is the primary surface for judging whether AMC v1 adds value under the frozen completion boundary.

Suggested workload properties:

- timestamped frames generated at fixed cadence
- small lookahead and strong freshness decay
- per-frame or per-chunk deadlines
- reliable QUIC stream transport for the main claim
- accounting for expired or stale payloads as misses rather than useful delivery

## Network scenario matrix

The benchmark matrix is fixed and preset-driven rather than dynamically varying within a single run.

Primary comparison shape:

- workload: `vod`, `live`
- controller: Quinn NewReno, Quinn Cubic, Quinn BBR, AMC preview
- network preset: named fixed `tc` profiles such as `wired_clean`, `wifi_moderate`, `wifi_unstable`, `lte_moderate`, and `lte_constrained`

Each preset must map to explicit shaping parameters rather than a marketing label alone. For every preset, record:

- base RTT
- delay jitter
- bottleneck bandwidth
- random loss rate
- queue limit or equivalent `tc` settings

Do not vary contention dynamically inside the main comparison matrix. Keep one fixed condition per cell so controller behavior can be attributed cleanly to that workload, controller, and preset combination.

Preferred implementation path:

- Linux `tc netem` for delay and loss
- `tbf` or `htb` for bandwidth limitation
- no full topology emulation unless the research question changes

Named presets should stay stable across reruns. If the project later studies adaptation to changing conditions, that should be a separate experiment family rather than part of the primary matrix.

Recommended host topology for the primary experiments:

- one Linux host
- client and server isolated by containers or network namespaces
- a Linux bridge or veth pair between them
- `tc` applied on that virtual link rather than on loopback
- distroless runtime images are fine when `tc` is configured by the host rather than from inside the container

Recommended control plane split:

- host-side runner owns container lifecycle and `tc` mutation
- raw transfer reports are written per run under `results/raw/`
- the harness analysis step consumes those raw reports and writes processed outputs under `results/processed/`

Validated operational path in the current repository:

- GCP Compute Engine Ubuntu host
- compose host-veth runner through `scripts/experiments/run_linux_vps_suite.sh` for `configs/harness/vps_fixed_preset_controller_matrix.json`
- direct host `harness run-suite` for `configs/harness/vps_host_live_coexistence_bbr_guardrail.json`
- `sudo` execution for both paths because host-veth discovery, namespace inspection, and `tc` mutation require root access
- `configs/harness/vps_baseline_vod_live.json` and `configs/harness/vps_demo_vod_live.json` as workflow-validation suites rather than final evidence

Use loopback only for smoke tests, local bring-up, and the current host-run fairness guardrail. Avoid using two different VPS instances for the main reported results because the public Internet path reduces repeatability.

Each experiment cell should be repeated with fixed seeds where applicable.

## Metrics

Collect both workload-facing and transport-facing metrics, but keep the primary story workload-specific.

The current locked metric split is:

- `live`: age of information, on-time delivery, stale or useless delivery ratio, latency, jitter
- `vod`: startup delay, stall or rebuffer behavior, useful delivery ratio, throughput relative to playback demand
- transport baseline for both: throughput, RTT summary, jitter, loss or recovery counters, completion success

### Transport-level metrics

- goodput
- RTT summary statistics
- jitter estimate
- retransmission or recovery-related counters, where exposed
- connection completion or failure outcome

### Application-level metrics

- VOD rebuffer ratio or stall count
- VOD startup delay
- live average age of information
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
- Coexistence and fairness checks are mandatory final evidence, but they should run as a separate experiment family with concurrent same-class flows on the same shaped link, not as replacements for the primary single-flow matrix.
- A fairness guardrail suite should report at least throughput share and Jain fairness index for a foreground controller against a baseline competing flow.
- The current canonical fairness suite is `configs/harness/vps_host_live_coexistence_bbr_guardrail.json` because the legacy docker VPS runner is still single-flow only.
- The current fairness evidence is therefore a documented topology exception: it runs through the host harness with `tc` on `lo` and should be interpreted as a separate guardrail family rather than as a topology-identical replacement for the fixed matrix.

Current AMC v1 reporting limit:

- reported AMC controller results correspond to a connection-wide congestion controller modulated by the latest sender utility sample
- they do not yet demonstrate per-stream semantic isolation, packet-level semantic annotations inside Quinn, or a full sender scheduling policy redesign
- any gain should therefore be described as evidence for the value of semantic-aware runtime signals under the present boundary, not as proof that the final AMC design space has been fully exercised
- repository completion is intentionally frozen at this AMC v1 boundary; AMC v2 remains explicit future work rather than an implicit completion requirement

## Primary benchmark scope

Primary controller comparisons:

- Quinn NewReno
- Quinn Cubic
- Quinn BBR
- AMC semantic-aware policy

Primary benchmark matrix:

- 2 workloads: `vod`, `live`
- 4 controllers: `new_reno`, `cubic`, `bbr`, `amc_preview`
- fixed named network presets backed by explicit `tc` parameters

This matrix replaces any plan to vary contention dynamically within a single benchmark run.

Primary benchmark questions:

- Does AMC improve multimedia utility under constrained networks?
- Does AMC remain acceptably fair when competing with Quinn baseline controllers?
- Which semantic inputs contribute most to any measured gain?

Frozen final-evidence configs for the repository:

- `configs/harness/vps_fixed_preset_controller_matrix.json` for the primary workload matrix
- `configs/harness/vps_host_live_coexistence_bbr_guardrail.json` for the mandatory fairness guardrail suite
- `configs/harness/local_controller_matrix.json` and `configs/harness/local_live_immediate_amc_bbr_coexistence.json` for required local parity coverage

Workflow-validation configs:

- `configs/harness/vps_baseline_vod_live.json` and `configs/harness/vps_demo_vod_live.json` for VPS bring-up and operator validation

Exploratory or non-canonical configs:

- `configs/harness/vps_live_realtime_controller_matrix.json`
- `configs/harness/vps_lte_constrained_live_matrix.json`
- `configs/harness/vps_live_coexistence_bbr_guardrail.json`, which is not runnable through the current docker runner

Use ablations where possible:

- transport-only baseline shell
- deadlines only
- deadlines plus importance
- deadlines plus importance plus dependency depth

## Experimental implementation plan

1. Establish a minimal Quinn client-server path.
2. Add open media download and preprocessing scripts under `scripts/media/`.
3. Add scenario configuration files for bandwidth, RTT, and loss.
4. Implement trace-driven VOD and live generators from preprocessed media artifacts.
5. Add baseline controller selection and result export.
6. Implement AMC controller and compare under the same matrix.
7. Produce processed tables and figures for the report.

## Artifact inputs

Recommended media artifact split:

- `data/raw/` for downloaded open source clips, ignored by Git
- `data/processed/segments/` for stream-friendly segment outputs
- `data/processed/manifests/` for replay manifests, semantic hints, and `ffprobe`-derived metadata

## Artifact organization

Recommended result split:

- `results/raw/` for per-run artifacts and machine-readable outputs
- `results/processed/` for aggregated CSV or JSON summaries
- `results/figures/` for plots used in the report

Current validated outputs from the Linux VPS path:

- raw per-run reports under `results/raw/harness/`
- per-run AMC analysis JSON under `results/processed/harness/`
- suite summary JSON under `results/processed/harness/*_summary.json`
- suite comparison JSON under `results/processed/harness/*_comparison.json`

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

The final report package should bind the frozen configs, exact reproduction commands, processed summaries and comparisons, required figures, bounded interpretation, and reproducibility notes into one reviewer-readable deliverable.