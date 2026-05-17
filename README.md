# quinn-amc

`quinn-amc` is a Rust workspace for prototyping and evaluating an application-aware multimedia congestion-control augment on top of QUIC using Quinn.

The immediate goal is to build a reproducible research codebase that can:

- express application semantics for multimedia workloads such as VOD and live delivery
- map those semantics onto a semantic-aware transport policy with a congestion-control core
- benchmark the custom approach against Quinn's built-in baseline controllers `new_reno`, `cubic`, and `bbr`
- generate figures and evidence suitable for a short conference-paper-style report

The current benchmark spec is fixed as:

- workload: `vod`, `live`
- controller: `new_reno`, `cubic`, `bbr`, `amc_preview`
- network preset: fixed named `tc` parameter bundles such as wired, WiFi, and LTE profiles

The repository is not currently using a dynamic adaptation suite inside the main benchmark matrix. One run corresponds to one fixed network preset.

## Phase 1 Frozen Completion Boundary

Phase 1 freezes repository completion around the current benchmarkable AMC v1 path.

- repository completion stops at AMC v1, which currently means a latest-sample, connection-wide runtime utility signal rather than per-stream or per-packet semantic isolation
- live traffic is the primary claim surface for AMC; VOD remains required supporting evidence for boundedness and comparability
- final reported evidence comes from the VPS path, with `configs/harness/vps_fixed_preset_controller_matrix.json` as the required workload matrix and `configs/harness/vps_host_live_coexistence_bbr_guardrail.json` as the required fairness guardrail suite
- local parity is still required through `configs/harness/local_controller_matrix.json` and `configs/harness/local_live_immediate_amc_bbr_coexistence.json`, but those runs support regression coverage rather than replacing the VPS evidence path
- `configs/harness/vps_baseline_vod_live.json` and `configs/harness/vps_demo_vod_live.json` remain workflow-validation suites, not final evidence
- `configs/harness/vps_live_realtime_controller_matrix.json`, `configs/harness/vps_lte_constrained_live_matrix.json`, and `configs/harness/vps_live_coexistence_bbr_guardrail.json` remain exploratory or non-canonical until a later phase promotes them explicitly
- the final deliverables are a frozen evidence set, a full figure package, a reviewer-readable report package, and a single-run ratatui introspection demo from a frozen raw report

## Phase 2 Frozen Workflow Model

Phase 2 freezes the repository around the workflow that actually exists today.

- the canonical VPS evidence path is split across two execution modes on the same GCP Linux VM
- the fixed-preset single-flow matrix runs through `scripts/experiments/run_linux_vps_suite.sh`, which applies `tc` on the demo-server container host-veth and then invokes `analyze-suite`
- the mandatory fairness guardrail runs directly through the host `harness` binary with `tc` applied to `lo`, because the legacy docker runner still launches exactly one foreground client per run
- local parity remains required, but it is a validation and regression layer rather than an alternative evidence path
- runner unification is deferred to Phase 3; Phase 2 documents the split explicitly instead of implying a workflow that does not exist

## Phase 4 AMC Milestone

Phase 4 freezes the controller milestone at AMC v1.

- AMC v1 is a latest-sample, connection-wide runtime utility design with sender-side semantic scoring and live ready-queue reordering
- repository completion does not require widening `RuntimeUtilityState` or adding AMC v2 state expansion
- the bounded claim is that AMC v1 helps the hardest constrained live cells relative to `new_reno` and `cubic` while staying throughput-fair against the required BBR guardrail
- the bounded limitation is that BBR remains the strongest overall baseline in the frozen matrix and AMC v1 is not claimed as a VOD startup winner

See [docs/amc-milestone.md](docs/amc-milestone.md) for the Phase 4 controller-completion criteria and evidence reading.

## Phase 5 Evidence Freeze

Phase 5 freezes the benchmark evidence set around the two canonical VPS suites already selected earlier in the roadmap.

- final evidence now means the processed outputs for `vps_fixed_preset_controller_matrix` and `vps_host_live_coexistence_bbr_guardrail`
- local parity outputs remain required regression support, but they are not part of the final evidence claim
- workflow-validation and exploratory outputs remain explicitly excluded from downstream figure and report work

See [docs/evidence-freeze.md](docs/evidence-freeze.md) for the exact included artifacts, excluded outputs, and reproduction commands that later phases must consume.

## Phase 6 Figure System

Phase 6 completes the final SVG figure set from the frozen Phase 5 comparison exports.

- the canonical figure workflow now renders only from `results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json` and `results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json`
- figure outputs under `results/figures/harness/` are now suite-prefixed so the fixed matrix and fairness guardrail families can live in one directory without collisions
- the validated final figure set contains `39` SVGs spanning live and VOD usefulness, deadline miss rate, throughput, delivery latency, jitter, live average age of information, VOD startup delay, VOD rebuffer ratio, foreground throughput share, fairness throughput ratio, and Jain fairness index

## Phase 7 Report Package

Phase 7 turns the frozen evidence and frozen figure set into a reviewer-readable report workflow.

- the canonical report now lives at `docs/final-report.md`
- the validated package command is `cargo run -p harness -- package-report`
- the validated generated package layout under `results/reports/final/` contains `report.md`, `manifest.json`, `reproducibility.md`, the four canonical processed artifacts, and the 39 frozen figures

## Phase 8 Live Demo

Phase 8 freezes the ratatui introspection viewer around one canonical constrained-live AMC showcase run.

- the canonical showcase raw report is `results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json`
- the validated demo command is `cargo run -p harness -- live-demo --report results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json --speed 1.0`
- the viewer now highlights current deadline status, delivery behavior, and the runtime utility telemetry already stored in the raw report

## Config Status

| Status | Configs | Current role |
| --- | --- | --- |
| final evidence | `configs/harness/vps_fixed_preset_controller_matrix.json` | canonical VPS workload matrix through the compose host-veth runner |
| final evidence | `configs/harness/vps_host_live_coexistence_bbr_guardrail.json` | canonical VPS fairness guardrail through direct host `harness run-suite` |
| local parity | `configs/harness/local_controller_matrix.json`, `configs/harness/local_live_immediate_amc_bbr_coexistence.json` | required regression and reproducibility support |
| workflow validation | `configs/harness/vps_baseline_vod_live.json`, `configs/harness/vps_demo_vod_live.json` | bring-up and operator-validation suites for the VPS path |
| workflow validation | `configs/harness/local_live_immediate_baselines.json`, `configs/harness/local_live_immediate_amc_preview.json`, `configs/harness/local_live_immediate_bbr_only.json` | focused local smoke suites for iteration |
| exploratory or non-canonical | `configs/harness/vps_live_realtime_controller_matrix.json`, `configs/harness/vps_lte_constrained_live_matrix.json`, `configs/harness/vps_live_coexistence_bbr_guardrail.json` | outside the frozen evidence set; `vps_live_coexistence_bbr_guardrail.json` is not runnable through the current docker runner |

## Core idea

The project does not claim to be a BBRv2 alternative.

The working contribution is a semantic-aware transport policy for user-space QUIC. The sender can use application-level semantics to influence transport decisions before congestion pressure turns all queued bytes into equally important work.

For this project, the important sender-visible signals are codec-agnostic primitives such as:

- deadline
- importance
- dependency depth
- freshness window

The primary implementation path uses QUIC streams for both VOD and live traffic. QUIC datagrams are intentionally out of the main claim and can be studied later only as a secondary experiment axis.

## Workspace layout

```text
crates/
  amc-core/      # congestion-control and application-semantic core logic
  demo-client/   # sender / traffic generator / experiment client
  demo-server/   # receiver / sink / experiment server
  harness/       # scenario orchestration, metrics collection, result export

data/
  raw/           # downloaded open media sources, kept out of Git
  processed/     # ffmpeg outputs such as CMAF-style fragments and replay manifests

scripts/
  experiments/   # Linux VPS runner invoked from within the GCP VM
  media/         # media download and preprocessing helpers

docs/
  core-idea.md   # thesis, scope, and contribution framing
  design.md      # semantic interface and transport-policy design
  evaluation.md  # benchmark questions, metrics, and scenario plan
  methodology.md # consolidated experiment and reporting plan
  replay-semantics.md # current preprocessing-to-semantic mapping rules

docker/
  demo-client.Dockerfile
  demo-server.Dockerfile
  harness.Dockerfile
  harness-tc.Dockerfile

.github/
  copilot-instructions.md
  logs/
```

## Current dependency stance

The project uses Quinn as a dependency, not a fork.

That is the correct starting point for this deadline because Quinn exposes custom congestion-control extension points through `quinn::congestion::{Controller, ControllerFactory}` and `TransportConfig::congestion_controller_factory(...)`.

Fork Quinn only if the public controller interface proves insufficient for the signals or hooks needed by the AMC design.

## Recommended toolchain workflow

Update Rust with `rustup`, which also updates `cargo` and `rustc` for the selected toolchain:

```powershell
rustup self update
rustup update stable
rustup component add rustfmt clippy
```

This repository pins Rust with `rust-toolchain.toml` for reproducible builds.

## GCP control plane

`gcloud` is the canonical interface for Compute Engine access in this repository.

Do not rely on repo-local SSH, bootstrap, or sync wrappers. Connect to the VM with `gcloud`, then work from the checked-out workspace on the VPS itself.

Typical flow:

1. Confirm the target VM and zone with `gcloud compute instances list`.
2. Connect with `gcloud compute ssh INSTANCE_NAME --zone ZONE`.
3. On the VM, change into the checked-out workspace.
4. Run the validated Linux runner commands from that shell.

If you need to copy files explicitly, use `gcloud compute scp` rather than repo-local sync helpers.

The validated remote path in this repository is a single GCP Linux VM with two documented execution modes under `sudo`: the compose host-veth runner for the fixed matrix and the direct host `harness` path for the fairness guardrail. Elevated privileges are required in both cases because `tc` mutation, namespace inspection, or loopback shaping need root access.

## Testing

Use this sequence when validating changes locally before any VPS run:

```powershell
cargo check
cargo test
```

Targeted validation commands that are useful during iteration:

```powershell
cargo run -p harness -- analyze-suite --config configs/harness/demo_vod_live.json
gcloud compute instances list
```

Workflow-validation VPS runs:

```bash
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_baseline_vod_live.json
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_demo_vod_live.json
```

Frozen final-evidence VPS workflow:

```bash
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_fixed_preset_controller_matrix.json
source "$HOME/.cargo/env"
cargo build -p harness
sudo ./target/debug/harness run-suite --config configs/harness/vps_host_live_coexistence_bbr_guardrail.json
sudo chown -R "$USER":"$USER" results
```

Frozen final-evidence figure workflow:

```powershell
cargo run -p harness -- plot-suite --comparison results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --output-dir results/figures/harness
cargo run -p harness -- plot-suite --comparison results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --output-dir results/figures/harness
```

Frozen final-evidence report workflow:

```powershell
cargo run -p harness -- package-report --report docs/final-report.md --matrix-comparison results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --fairness-comparison results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --figure-dir results/figures/harness --output-dir results/reports/final
```

Frozen final-evidence live-demo workflow:

```powershell
cargo run -p harness -- live-demo --report results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json --speed 1.0
```

The first command is the canonical single-flow matrix path. The second command sequence is the canonical fairness guardrail path. They intentionally use different execution modes in the current repository because `run_linux_vps_suite.sh` is still single-client only.

The compose VPS runner now rejects configs that contain `runs[*].coexistence` and validates the referenced replay manifest before launching any container work. That preflight checks the manifest file itself, the init segment, every referenced media segment, each declared `size_bytes`, and whether the manifest is stale relative to any referenced segment payload.

Required local parity runs:

```powershell
cargo run -p harness -- run-suite --config configs/harness/local_controller_matrix.json
cargo run -p harness -- run-suite --config configs/harness/local_live_immediate_amc_bbr_coexistence.json
```

The VPS runner now normalizes `results/` ownership back to the invoking user when run through `sudo`, so processed artifacts should not remain owned by `root` after a successful suite run.

The Docker build path also defaults to BuildKit via the runner, and the service Dockerfiles use cache mounts for Cargo registry, git, and target state to reduce repeated VPS rebuild cost.

The VPS suite runner now also keeps a build stamp under `results/` and skips `docker compose build` when the tracked source inputs are older than the cached service images. It also writes per-run diagnostics under `results/raw/harness/runner/`, including client logs, recent server logs, tc qdisc snapshots, and simple timing logs.

The current organization policy disables service account key creation, so the validated remote update path is manual rather than GitHub Actions based. Use `gcloud compute ssh` to access the VM and update the checkout in place with `git pull` when possible, or copy a prepared repository archive with `gcloud compute scp` when the host cannot pull directly from GitHub.

The clean-host bootstrap path is:

```bash
cd /home/leven/quinn-amc
bash scripts/experiments/bootstrap_linux_vps.sh
```

Direct harness behavior is split intentionally:

- `cargo run -p harness -- run-suite --config ...` generates raw reports and processed outputs
- `cargo run -p harness -- analyze-suite --config ...` only analyzes already-existing raw reports, skips missing matrix cells, and fails only if no raw reports are available at all
- `cargo run -p harness -- plot-suite --comparison ... --output-dir results/figures/harness` renders suite-prefixed overview and scenario-grouped SVG figures from a comparison export
- `cargo run -p harness -- package-report ...` validates the frozen comparison exports and figure inventory, then assembles a Markdown-first report package under `results/reports/final/`
- `cargo run -p harness -- live-demo --report ...` replays a raw report in a ratatui dashboard so the controller and utility signals can be inspected live

The suite run path now avoids several fixed per-cell costs:

- one server endpoint and one self-signed certificate are reused across all runs in a suite
- replay manifests and segment payload bytes are loaded once per suite and reused across controllers
- freshly generated raw reports stay in memory for immediate AMC analysis instead of being written and reread first
- internal segment headers use a compact binary encoding, while human-facing artifacts remain JSON
- `run-suite` skips transport for runs whose raw report is newer than the suite config and replay manifest, so repeated iterations only rerun stale cells

Processed harness outputs now also carry input provenance:

- suite summaries and comparison exports include SHA-256 digests for the suite config and replay manifest
- per-run AMC analyses include the same suite inputs plus the raw report digest used to produce that analysis
- replay-manifest freshness checks include referenced init and media segments, not just the manifest JSON timestamp

Expected VPS outputs:

- `results/raw/harness/*_report.json`
- `results/processed/harness/*_amc.json`
- `results/processed/harness/*_summary.json`
- `results/processed/harness/*_comparison.json`
- `results/figures/harness/*.svg` after plotting the two canonical comparison exports, including suite-aware files such as `vps_fixed_preset_controller_matrix_overview_throughput_mbps.svg` and `vps_host_live_coexistence_bbr_guardrail_live_realtime_fairness_jain_index.svg`
- `results/reports/final/` after `package-report`, including `report.md`, `manifest.json`, `reproducibility.md`, `artifacts/*.json`, and `figures/*.svg`
- `results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json` as the canonical Phase 8 showcase raw report for `live-demo`

Fairness and coexistence runs are a separate experiment family. The harness now supports an optional concurrent competitor flow per run and records the competitor report plus fairness metrics in the suite summary and comparison export. A practical local smoke config is `configs/harness/local_live_immediate_amc_bbr_coexistence.json`.
Fairness and coexistence are mandatory final evidence for the repository, but the currently canonical VPS path is `configs/harness/vps_host_live_coexistence_bbr_guardrail.json`. The legacy docker-runner-oriented `configs/harness/vps_live_coexistence_bbr_guardrail.json` remains non-canonical and unsupported under the current runner until the host-veth path can launch both foreground and competitor clients in one suite.

## Quinn feature selection

The workspace pins Quinn with this feature set:

- `runtime-tokio`: aligns with the async runtime we are likely to use for demos and orchestration
- `rustls-ring` and `ring`: stable TLS/crypto path for local experiments
- `platform-verifier`: useful when clients need platform certificate verification
- `log`: keeps protocol logging available during early bring-up

Not enabled by default here:

- `bloom`: acceptable but unnecessary for the current milestone
- `qlog`: useful later if packet-level trace export becomes part of the evaluation

## Methodology

See the project notes under `docs/`:

- [docs/core-idea.md](docs/core-idea.md) for the thesis and scope boundaries
- [docs/design.md](docs/design.md) for the application-to-transport semantic interface
- [docs/amc-milestone.md](docs/amc-milestone.md) for the frozen AMC v1 completion boundary and evidence reading
- [docs/evidence-freeze.md](docs/evidence-freeze.md) for the frozen Phase 5 artifact inventory and reproduction contract
- [docs/evaluation.md](docs/evaluation.md) for benchmark questions and metrics
- [docs/final-report.md](docs/final-report.md) for the final reviewer-readable report bound to the frozen evidence set
- [docs/live-demo.md](docs/live-demo.md) for the canonical Phase 8 showcase run and operator guide
- [docs/methodology.md](docs/methodology.md) for the consolidated experiment plan

## Data pipeline

The media path should stay trace-driven and reproducible:

1. Download a small set of openly licensed source clips into `data/raw/`.
2. Use `ffmpeg` to create CMAF-style fragmented MP4 outputs and a DASH manifest offline.
3. Generate a lightweight replay manifest that the client can consume without embedding a full player stack.
4. Attach lightweight semantic hints during preprocessing so the harness can score replay units against `amc-core`.
5. Keep the runtime sender focused on segment replay and semantic metadata, not container parsing or transcoding.

For path shaping, prefer Linux `tc netem` plus a rate limiter such as `tbf` or `htb`. That gives controllable end-to-end behavior without building a full topology model.

## Near-term implementation plan

1. Extend the working Quinn demo client and server path into timed replay of preprocessed CMAF segments.
2. Add `ffmpeg` and `ffprobe`-based preprocessing scripts and source media handling under `data/`.
3. Define an application-to-transport semantic interface for `vod` and `live` traffic classes.
4. Implement baseline runs with Quinn-provided congestion controllers.
5. Add the AMC policy and congestion-control core under the same scenario matrix.
6. Export processed results and figures for the final report.

## Demo run

The current demo binaries establish a Quinn connection over QUIC and can transfer a full preprocessed CMAF-style asset from the client to the server using a replay manifest. The server also records per-segment arrival timing and usefulness observations into a raw JSON report.

Start the server in one terminal:

```powershell
cargo run -p demo-server -- --bind 127.0.0.1:5001 --cert-out demo-cert.der
```

Then run the client in a second terminal:

```powershell
cargo run -p demo-client -- --server 127.0.0.1:5001 --cert demo-cert.der --replay-manifest data/processed/manifests/big_buck_bunny_replay.json --pace realtime --mode live
```

Expected behavior:

- the server writes `demo-cert.der`, accepts one connection, receives the init segment plus all media fragments, writes a raw JSON report under `results/raw/`, and replies with a JSON transfer summary
- the client connects using `localhost` as the certificate name, reads the replay manifest, sends the full processed stream, and logs the returned summary including useful versus late media segments

## Harness run

The harness now runs a config-defined suite so both `vod` and `live` replay modes can be exercised back to back against the same processed asset.

The default suite config lives at `configs/harness/demo_vod_live.json` and defines:

- a `vod_realtime` run
- a `live_realtime` run
- per-run baseline controller selection
- named network scenarios, including a Linux `tc netem` placeholder profile for later impaired runs
- a shared semantic profile that complements per-segment semantic hints stored in the replay manifest
- a shared replay manifest and results root

Run it with:

```powershell
cargo run -p harness -- run-suite --config configs/harness/demo_vod_live.json
```

Expected outputs:

- raw per-run server reports under `results/raw/harness/`
- one suite summary under `results/processed/harness/demo_vod_live_summary.json`
- one per-run AMC analysis under `results/processed/harness/*_amc.json`

Each run now carries a `controller` field in harness config, and raw plus processed outputs record the selected baseline controller.

The controller set is intentionally limited to Quinn's current built-ins plus AMC preview:

- `new_reno` via `quinn::congestion::NewRenoConfig`
- `cubic` via `quinn::congestion::CubicConfig`
- `bbr` via `quinn::congestion::BbrConfig`
- `amc_preview` as the only custom controller in this repository

The comparison export now includes workload-oriented metrics such as throughput, delivery latency, jitter, deadline miss rate, and live average age of information. Partial controller matrices are preserved in the export with explicit `missing_controllers` metadata instead of aborting the entire analysis pass. The plotter keeps suite-prefixed overview charts and scenario-grouped controller comparison figures per mode and pace, and fairness-enabled comparison exports also emit throughput-share, throughput-ratio, and Jain-fairness figures.

For VOD, the processed comparison export now also records continuity-oriented metrics from a simple buffered playout model: startup delay, rebuffer count, rebuffer duration, and rebuffer ratio. This keeps VOD comparisons informative even when all controllers eventually deliver every segment.

For a Windows-safe local controller sweep that produces real data across all four controllers, run:

```powershell
cargo run -p harness -- run-suite --config configs/harness/local_controller_matrix.json
cargo run -p harness -- plot-suite --comparison results/processed/harness/local_controller_matrix_comparison.json
```

For the primary benchmark path, harness suites should be organized as workload × controller × network preset matrices rather than ad hoc one-off runs. The frozen final-evidence matrix is `configs/harness/vps_fixed_preset_controller_matrix.json`.

The AMC analysis now prefers semantic hints from preprocessing artifacts and only falls back to harness defaults when a manifest does not provide them.

Current limitation:

- the harness records named `linux_tc_netem` scenarios in config and output, but does not apply them yet on this Windows host; actual `tc` orchestration should be added when running on Linux

## Linux Experiment Topology

For the primary experiments, use one Linux host, not two different VPS instances.

Preferred setup:

- run client and server in separate containers or network namespaces on the same Linux machine
- connect them through a Linux bridge or veth pair
- apply `tc netem` and `tbf` on that virtual link

Avoid for the main claim:

- two separate VPS hosts over the public Internet, because uncontrolled path variation hurts reproducibility
- `tc` on loopback as the primary single-flow matrix setup, because loopback behavior is too special to trust as the main benchmark path

Loopback is still acceptable for smoke tests, local bring-up, and the current host-run fairness guardrail until the VPS runner gains coexistence coverage.

## Container Images

Container build files are available under `docker/` for Linux-host deployment:

```powershell
docker build -f docker/demo-client.Dockerfile -t quinn-amc/demo-client .
docker build -f docker/demo-server.Dockerfile -t quinn-amc/demo-server .
docker build -f docker/harness.Dockerfile -t quinn-amc/harness .
```

Practical note:

- the default client, server, and harness images are distroless and assume network shaping is applied externally by the Linux host or VPS
- if you want the harness container itself to execute `tc`, build `docker/harness-tc.Dockerfile` instead and grant it `NET_ADMIN`
- processed manifests and segments should be mounted into `/workspace/data`, and results should be mounted from `/workspace/results`

`tc` requires Linux. It is part of Linux traffic control and depends on Linux qdisc support in the kernel. So if the harness is the component applying impairment, it must run on Linux and have access to the target Linux interface. Client and server do not inherently need Linux for basic replay, but the shaped experiment topology does.

Recommended default:

- keep all three runtime images distroless
- configure `tc` on the Linux VPS host or on host-owned namespaces/bridges
- let the harness only record and analyze the named scenario, not mutate qdisc state from inside the container

That is the cleaner setup for reproducible experiments.

## Compose

`compose.yaml` defines a shared Docker network for the distroless images.

Examples:

```powershell
docker compose --profile harness up --build harness
docker compose --profile demo-server up --build demo-server
docker compose --profile demo-client up --build demo-client
```

Notes:

- the harness service runs the config-driven suite inside a single container because the current harness links the demo client and server as libraries
- the standalone demo client and server services are for smoke tests and manual replay runs
- the demo client depends on the server, but you should still let the server start and write its certificate before launching the client profile in a real run

## VPS Architecture

The current canonical Linux VPS workflow is split across two paths on the same VM.

Fixed-preset matrix path:

1. The VPS host owns `tc` and applies shaping on the server container host-veth.
2. The demo server and demo client run as isolated containers on the same Docker bridge network.
3. The host-side experiment runner iterates the scenario matrix: apply `tc`, run server and client, collect raw report, clear `tc`, move to the next run.
4. The distroless harness container runs only the post-run `analyze-suite` step over the collected raw reports.

Fairness guardrail path:

1. The VPS host runs `cargo build -p harness` and then invokes the host `harness` binary directly under `sudo`.
2. The harness executes both foreground and competitor flows from one host process because the current docker runner cannot orchestrate concurrent clients.
3. `tc` is applied to `lo` for this guardrail path, and the resulting evidence should be interpreted as a separate fairness family rather than a topology-identical replacement for the fixed matrix.

That means the current `harness` binary has two roles:

- `run-suite`: local in-process orchestration for development on one machine
- `analyze-suite`: offline processing of raw reports produced by the VPS host runner

For the Linux VPS flow, connect to the VM with `gcloud compute ssh`, then use the host-side runner from the checked-out workspace:

```bash
bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_demo_vod_live.json
```

For the currently validated GCP VM path, invoke the runner with `sudo`:

```bash
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_baseline_vod_live.json
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_demo_vod_live.json
```

The VPS config at `configs/harness/vps_demo_vod_live.json` remains the starting point for bringing up the host-veth runner path, but it is not final evidence.

Unsupported under the current docker runner:

- `configs/harness/vps_live_coexistence_bbr_guardrail.json`, because the script launches one foreground demo-client container per run and cannot emit coexistence raw reports

Current bootstrap limitation:

- the host runner applies `tc` on the server container host-veth, which matches the main client-to-server media flow and is a reasonable first controlled path for this upload-style experiment
- if later you need explicitly symmetric shaping, extend the host runner to pre-create and shape both endpoint links or move to host-managed namespaces with paired veth links
- the host-veth docker runner does not yet support coexistence execution, so the host-run harness path is the current canonical fairness workflow

## Build status

The workspace structure is bootstrapped, the media pipeline now targets CMAF-style fragments plus replay manifests, and the demo client/server path can transfer a full processed asset over Quinn.

The single-host Linux VPS path is also validated on GCP for all currently documented workflow layers:

- baseline-only replay through `configs/harness/vps_baseline_vod_live.json`
- impaired host-managed `tc` replay through `configs/harness/vps_demo_vod_live.json`
- fixed-preset final-evidence replay through `configs/harness/vps_fixed_preset_controller_matrix.json`
- host-run fairness guardrail replay through `configs/harness/vps_host_live_coexistence_bbr_guardrail.json`

See `TODO.md` for the remaining cleanup and experiment-expansion work.