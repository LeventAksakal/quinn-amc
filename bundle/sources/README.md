# quinn-amc

`quinn-amc` is a Rust research workspace for a semantic-aware multimedia congestion-control augment built on Quinn.

## Status

The repository is complete for its AMC v1 scope.

That completion boundary is deliberately narrow:

- AMC v1 improves the hardest constrained live cells relative to `new_reno` and `cubic`
- BBR remains the strongest overall live baseline in the fixed matrix
- VOD is required supporting evidence, but AMC v1 is not a startup-delay winner
- fairness is required and is interpreted at the throughput-sharing level against BBR

The repository is not claiming AMC v2, docker-runner coexistence parity, or broad BBR replacement.

## Artifact Layout

The repository now separates local and VPS outputs explicitly:

- `results/local/` for Windows-safe local runs, smoke suites, and local parity artifacts
- `results/vps/` for canonical VPS suite outputs and files copied back from `quinn-amc-vps`

The canonical evidence, figures, packaged report, and live-demo artifact all belong under `results/vps/`.

## Workspace Layout

```text
crates/
  amc-core/      congestion-control and application-semantic logic
  demo-client/   sender and replay traffic generator
  demo-server/   receiver and transfer sink
  harness/       suite orchestration, analysis, plotting, packaging, live demo

configs/harness/ final-evidence, parity, validation, and exploratory suites
scripts/         VPS runner and media preprocessing scripts
docs/            canonical methodology, report, and focused appendices
data/            raw media and processed manifests or segments
results/         split local and VPS output trees
```

## Canonical Workflows

### Local validation

```powershell
cargo check
cargo test
```

### Local parity suites

```powershell
cargo run -p harness -- run-suite --config configs/harness/local_controller_matrix.json
cargo run -p harness -- run-suite --config configs/harness/local_live_immediate_amc_bbr_coexistence.json
```

Use `analyze-suite` only when the corresponding raw reports already exist:

```powershell
cargo run -p harness -- analyze-suite --config configs/harness/local_controller_matrix.json
```

These suites now write under `results/local/`.

### Canonical VPS fixed matrix

```bash
cd /home/leven/quinn-amc
sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_fixed_preset_controller_matrix.json
```

### Canonical VPS fairness guardrail

```bash
cd /home/leven/quinn-amc
source "$HOME/.cargo/env"
cargo build -p harness
sudo ./target/debug/harness run-suite --config configs/harness/vps_host_live_coexistence_bbr_guardrail.json
sudo chown -R "$USER":"$USER" results/vps
```

### Figure regeneration

```powershell
cargo run -p harness -- plot-suite --comparison results/vps/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --output-dir results/vps/figures/harness
cargo run -p harness -- plot-suite --comparison results/vps/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --output-dir results/vps/figures/harness
```

### Final report package

```powershell
cargo run -p harness -- package-report --report docs/final-report.md --matrix-comparison results/vps/processed/harness/vps_fixed_preset_controller_matrix_comparison.json --fairness-comparison results/vps/processed/harness/vps_host_live_coexistence_bbr_guardrail_comparison.json --figure-dir results/vps/figures/harness --output-dir results/vps/reports/final
```

### Canonical live demo

```powershell
cargo run -p harness -- live-demo --report results/vps/raw/harness/live_realtime_amc_preview_lte_constrained_report.json --speed 1.0
```

### VPS access and artifact copy

```powershell
gcloud compute instances list
gcloud compute ssh quinn-amc-vps --zone europe-west6-c
gcloud compute scp --recurse quinn-amc-vps:/home/leven/quinn-amc/results/vps results --zone europe-west6-c
```

## Config Roles

| Status | Configs | Current role |
| --- | --- | --- |
| final evidence | `configs/harness/vps_fixed_preset_controller_matrix.json` | canonical VPS workload matrix through the compose host-veth runner |
| final evidence | `configs/harness/vps_host_live_coexistence_bbr_guardrail.json` | canonical VPS fairness guardrail through direct host `harness run-suite` |
| local parity | `configs/harness/local_controller_matrix.json`, `configs/harness/local_live_immediate_amc_bbr_coexistence.json` | required regression and reproducibility support |
| workflow validation | `configs/harness/vps_baseline_vod_live.json`, `configs/harness/vps_demo_vod_live.json`, `configs/harness/local_live_immediate_baselines.json`, `configs/harness/local_live_immediate_amc_preview.json`, `configs/harness/local_live_immediate_bbr_only.json` | operator-validation and focused smoke suites |
| exploratory or non-canonical | `configs/harness/vps_live_realtime_controller_matrix.json`, `configs/harness/vps_lte_constrained_live_matrix.json`, `configs/harness/vps_live_coexistence_bbr_guardrail.json` | outside the frozen evidence set; the docker runner still cannot emit coexistence raw reports |

## Output Locations

- local run reports and analyses: `results/local/`
- VPS raw reports, processed analyses, figures, and packaged reports: `results/vps/`

## Canonical Documents

- `README.md`: operator guide and current repository status
- `docs/methodology.md`: canonical scope, workflow, evidence, and reproduction spec
- `docs/final-report.md`: bounded final claim and frozen result interpretation
- `docs/result-schema.md`: raw and processed artifact schema
- `docs/replay-semantics.md`: replay-manifest semantic-hint rules
- `docs/live-demo.md`: canonical live-demo workflow

## Notes

- The project uses Quinn as a dependency, not a fork.
- The main claim uses QUIC streams for both VOD and live traffic.
- QUIC datagrams remain outside the primary evaluation boundary.
- `tc` requires Linux; Windows is fine for local cargo validation and report/demo inspection, not for canonical shaped experiments.

See `TODO.md` for integrity guards and future work outside repository completion.