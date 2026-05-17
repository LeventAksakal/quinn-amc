# quinn-amc

`quinn-amc` is a Rust research workspace for a semantic-aware multimedia congestion-control augment built on Quinn.

The repository is complete at the AMC v1 boundary. It does not claim to replace BBR or to cover the broader AMC v2 design space.

## Project definition

The project evaluates whether sender-visible multimedia semantics can improve application outcomes over baseline QUIC congestion controllers.

The fixed benchmark surface is:

- workloads: `vod`, `live`
- controllers: `new_reno`, `cubic`, `bbr`, `amc_preview`
- network presets: fixed named `tc` profiles such as wired, WiFi, and LTE variants

The bounded repository claim is:

- AMC v1 improves the hardest constrained live cells relative to `new_reno` and `cubic`
- BBR remains the strongest overall live baseline in the frozen matrix
- VOD remains required supporting evidence, but AMC v1 is not a startup-delay winner
- fairness is required and is interpreted at the throughput-sharing level against BBR

The primary semantic inputs are codec-agnostic transport hints such as deadline, importance, dependency depth, and freshness window. The main claim uses QUIC streams for both VOD and live traffic. QUIC datagrams remain outside the primary evaluation boundary.

## Repository status

This repository is finished for its intended AMC v1 scope.

The final deliverables are:

- frozen VPS evidence for the fixed matrix and fairness guardrail
- a full figure set under `results/figures/harness/`
- a Markdown-first report package under `results/reports/final/`
- a single-run ratatui live demo driven by one frozen raw report

Anything beyond that boundary belongs to future work. See [TODO.md](TODO.md).

## Workspace layout

```text
crates/
  amc-core/      congestion-control and application-semantic core logic
  demo-client/   sender and replay traffic generator
  demo-server/   receiver and transfer sink
  harness/       suite orchestration, analysis, plotting, report packaging, live demo

configs/harness/ final evidence, parity, validation, and exploratory suite configs
scripts/         VPS runner and media preprocessing scripts
docs/            methodology, evidence notes, report, and operator guidance
data/            raw media and processed manifests or segments
results/         local raw, processed, figure, and packaged outputs
```

## How to use the project

### Prerequisites

- Rust via `rustup`; the repo pins the toolchain in `rust-toolchain.toml`
- `cargo`
- `gcloud` for the canonical VPS workflow
- Linux `tc` support on the VPS for shaped runs

Recommended Rust setup:

```powershell
rustup self update
rustup update stable
rustup component add rustfmt clippy
```

### Local validation

Use this as the default local sanity check:

```powershell
cargo check
cargo test
```

### Local benchmark path

Run the local controller matrix:

```powershell
cargo run -p harness -- run-suite --config configs/harness/local_controller_matrix.json
```

Run the local coexistence parity suite:

```powershell
cargo run -p harness -- run-suite --config configs/harness/local_live_immediate_amc_bbr_coexistence.json
```

Use `analyze-suite` only when the corresponding raw reports already exist:

```powershell
cargo run -p harness -- analyze-suite --config configs/harness/local_controller_matrix.json
```

### Figure generation

Render the frozen final figures from the canonical comparison exports:

```powershell
cargo run -p harness -- plot-suite --comparison results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --output-dir results/figures/harness
cargo run -p harness -- plot-suite --comparison results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --output-dir results/figures/harness
```

### Report packaging

Assemble the final reviewer-readable package:

```powershell
cargo run -p harness -- package-report --report docs/final-report.md --matrix-comparison results/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --fairness-comparison results/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --figure-dir results/figures/harness --output-dir results/reports/final
```

The packaged outputs land under `results/reports/final/`:

- `report.md`
- `manifest.json`
- `reproducibility.md`
- `artifacts/*.json`
- `figures/*.svg`

### Live demo

Launch the canonical showcase demo:

```powershell
cargo run -p harness -- live-demo --report results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json --speed 1.0
```

The live demo is intentionally single-run only. It now shows both the AMC signal inputs and, when the report carries the richer schema, the controller snapshot with phase, last event, window, threshold, and growth-step state. See [docs/live-demo.md](docs/live-demo.md).

### Standalone demo client and server

Start the server:

```powershell
cargo run -p demo-server -- --bind 127.0.0.1:5001 --cert-out demo-cert.der
```

Run the client in another terminal:

```powershell
cargo run -p demo-client -- --server 127.0.0.1:5001 --cert demo-cert.der --replay-manifest data/processed/manifests/big_buck_bunny_replay.json --pace realtime --mode live
```

## Canonical VPS workflow

The canonical evidence path is split across two execution modes on one GCP Linux VM.

### Fixed matrix

```bash
cd /home/leven/quinn-amc
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_fixed_preset_controller_matrix.json
```

### Fairness guardrail

```bash
cd /home/leven/quinn-amc
source "$HOME/.cargo/env"
cargo build -p harness
sudo ./target/debug/harness run-suite --config configs/harness/vps_host_live_coexistence_bbr_guardrail.json
sudo chown -R "$USER":"$USER" results
```

Use `gcloud` as the canonical control plane:

```powershell
gcloud compute instances list
gcloud compute ssh quinn-amc-vps --zone europe-west6-c
```

If you need to copy artifacts explicitly:

```powershell
gcloud compute scp quinn-amc-vps:/home/leven/quinn-amc/results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json results/raw/harness/live_realtime_amc_preview_lte_constrained_report.json --zone europe-west6-c
```

## Config roles

| Status | Configs | Current role |
| --- | --- | --- |
| final evidence | `configs/harness/vps_fixed_preset_controller_matrix.json` | canonical VPS workload matrix through the compose host-veth runner |
| final evidence | `configs/harness/vps_host_live_coexistence_bbr_guardrail.json` | canonical VPS fairness guardrail through direct host `harness run-suite` |
| local parity | `configs/harness/local_controller_matrix.json`, `configs/harness/local_live_immediate_amc_bbr_coexistence.json` | required regression and reproducibility support |
| workflow validation | `configs/harness/vps_baseline_vod_live.json`, `configs/harness/vps_demo_vod_live.json` | bring-up and operator-validation suites for the VPS path |
| workflow validation | `configs/harness/local_live_immediate_baselines.json`, `configs/harness/local_live_immediate_amc_preview.json`, `configs/harness/local_live_immediate_bbr_only.json` | focused local smoke suites for iteration |
| exploratory or non-canonical | `configs/harness/vps_live_realtime_controller_matrix.json`, `configs/harness/vps_lte_constrained_live_matrix.json`, `configs/harness/vps_live_coexistence_bbr_guardrail.json` | outside the frozen evidence set; `vps_live_coexistence_bbr_guardrail.json` is not runnable through the current docker runner |

## Output locations

Generated outputs are local workspace artifacts. They are not the main versioned surface of the repository.

- raw run reports: `results/raw/harness/`
- processed analyses and comparisons: `results/processed/harness/`
- final figures: `results/figures/harness/`
- packaged report deliverable: `results/reports/final/`

## Key docs

- [docs/final-report.md](docs/final-report.md) for the final bounded claim and reviewer-readable report
- [docs/evidence-freeze.md](docs/evidence-freeze.md) for the canonical artifact and reproduction boundary
- [docs/methodology.md](docs/methodology.md) for the full experimental and reporting model
- [docs/amc-milestone.md](docs/amc-milestone.md) for the AMC v1 boundary
- [docs/live-demo.md](docs/live-demo.md) for the canonical demo workflow
- [docs/vps-results-handoff.md](docs/vps-results-handoff.md) for the VPS artifact handoff context

## Notes

- The project uses Quinn as a dependency, not a fork.
- Keep results and evidence interpretation tied to the frozen `vps_*` artifacts unless you explicitly reopen scope.
- Use streams for the main claim. Do not widen the main evaluation to QUIC datagrams without a deliberate scope change.

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