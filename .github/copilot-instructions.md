## Agent Behavior

- For any substantial feature implementation, refactor, fix, or chore, keep a log book under `.github/logs/*` with files named `dd_mm_yyyy.md`.

# Project Guidelines

** This is a living document, not a static spec. It should evolve as the project evolves, and it should be updated whenever the project shape, scope, or workflow changes. **

## Scope

- This repository is a Rust research workspace for a QUIC congestion-control augment built on Quinn.
- Favor reproducible experiments and clear methodology over broad feature work.
- Treat the codebase as a benchmarkable systems project, not a generic app scaffold.

## Architecture

- The repository root is a Cargo workspace with four crates under `crates/`.
- `amc-core` owns application-semantics and transport-policy logic.
- `demo-client` and `demo-server` own experiment traffic generation and sink behavior.
- `harness` owns scenario definitions, run orchestration, metrics export, and result packaging.

## Knowledge Tools

### codebase-memo (primary — use first for all code work)

- **Always call `mcp_codebase-memo_index_status` before any feature work** to confirm the index is fresh; re-index with `mcp_codebase-memo_index_repository` (mode `full`, project `c-Code-quinn-amc`) if stale.
- Use `mcp_codebase-memo_search_code` and `mcp_codebase-memo_get_code_snippet` for context gathering before reading files or running greps — prefer the graph over raw file reads.
- Use `mcp_codebase-memo_get_architecture` at the start of any feature or refactor to understand crate boundaries and dependencies.
- Use `mcp_codebase-memo_trace_path` to understand call chains and data flows before changing interfaces.
- Use `mcp_codebase-memo_detect_changes` to scope the blast radius of a proposed change before touching code.
- Use `mcp_codebase-memo_query_graph` and `mcp_codebase-memo_search_graph` for cross-cutting queries (e.g. all callers of a function, all types implementing a trait).
- Use `mcp_codebase-memo_manage_adr` to record and retrieve architectural decisions.
- After landing a substantial change, call `mcp_codebase-memo_index_repository` (mode `moderate`) to keep the index current.

### context7 (use for all library and API questions)

- **Always resolve a library with `mcp_context7_resolve-library-id` before calling `mcp_context7_query-docs`.**
- Use context7 for any question about Quinn, tokio, rustls, cargo, tc netem, gcloud CLI, or any other dependency — even well-known APIs. Training data is stale; prefer live docs.
- Call context7 before proposing a new dependency, configuration option, or API usage pattern to verify current signatures, defaults, and breaking changes.
- Do not guess at crate feature flags, version compatibility, or migration paths — fetch the docs first.

### Combined workflow

1. **Plan**: `get_architecture` → `search_code` → context7 docs for relevant crates.
2. **Implement**: `trace_path` / `query_graph` to verify blast radius → edit files → `cargo check`.
3. **Validate**: `detect_changes` to confirm scope → `cargo test` → re-index (moderate) if structure changed.
4. **Record**: `manage_adr` for any non-obvious design choice; log book entry under `.github/logs/`.

## Tooling

- Agents should treat the local CLI toolchain as part of the working environment, not as an external afterthought.
- Use `cargo` for local Rust validation and `gcloud` for Compute Engine operations. Treat `gcloud` as the canonical control plane for VPS access.
- Use `gh` when authenticated for GitHub repository operations such as inspecting remotes, pull requests, issues, workflow runs, and repository metadata.
- Use `gcloud` when authenticated for Google Cloud operations such as listing Compute Engine instances, SSH access, file copy, and VM bootstrap for the validated single-host VPS path.
- Do not add or rely on repo-local SSH, bootstrap, or sync wrappers when direct `gcloud compute ssh` or `gcloud compute scp` is sufficient.
- The current organization policy disables service account key creation (`iam.disableServiceAccountKeyCreation`), so do not assume GitHub Actions service-account-key authentication is available for VM sync.
- Prefer the manual operator path for remote updates: `gcloud compute ssh` into the VM, `git pull` in the checked-out repository when the host can reach GitHub, or `gcloud compute scp` to copy a prepared archive when direct pull is not viable.
- When a workflow depends on `gh` or `gcloud`, keep the corresponding documentation and repo instructions synchronized with the validated command path.

## Quinn usage

- Start from the public Quinn API and only propose a Quinn fork if the required congestion-control hooks are not exposed through `quinn::congestion` or `TransportConfig`.
- Keep congestion-control changes isolated behind explicit interfaces so baseline controllers and AMC runs can share the same harness.
- Preserve comparability: baseline controllers and AMC should run under the same workload and network scenario definitions.
- The core contribution is a semantic-aware transport policy with a congestion-control core, not a claim of building a BBRv2 replacement.

## Experiment design

- Optimize for repeatability and traceability.
- Prefer trace-driven or synthetic multimedia workloads over full media stacks unless realism clearly improves the evaluation.
- Use Linux `tc netem` as the preferred path-shaping mechanism for reproducible RTT, loss, and bandwidth control. Do not model full network topologies unless topology itself becomes part of the research question.
- The current canonical VPS evidence model is split on one GCP Linux VM: `configs/harness/vps_fixed_preset_controller_matrix.json` runs through `scripts/experiments/run_linux_vps_suite.sh` with host-managed `tc` on the demo-server container host-veth, while `configs/harness/vps_host_live_coexistence_bbr_guardrail.json` runs through direct host `harness run-suite` with `tc` on `lo` because the docker runner is still single-flow only.
- Treat `configs/harness/vps_baseline_vod_live.json` and `configs/harness/vps_demo_vod_live.json` as workflow-validation suites, not final evidence.
- Treat `configs/harness/vps_live_coexistence_bbr_guardrail.json` as non-canonical until the VPS docker runner can emit coexistence raw reports.
- Keep one semantic traffic class per connection unless mixed-traffic behavior is the specific subject under test.
- Every benchmark change should state which metric, scenario, or hypothesis it affects.
- Use streams for both VOD and live traffic in the main claim. Keep QUIC datagrams out of the primary evaluation unless they are a separate experimental axis.
- The application-to-transport interface should expose codec-agnostic semantic inputs such as deadline, importance, dependency depth, and freshness window.
- Do not assume raw codec labels such as GOP or frame type are directly meaningful to the congestion-control core without being translated into sender-visible utility signals.
- Prefer an offline `ffmpeg` and `ffprobe` preprocessing pipeline that converts open media assets into replayable segment sets and semantic traces rather than parsing full media stacks in the runtime sender.
- Keep large source media under `data/raw/` and out of Git. Small derived manifests or traces may be versioned only if they are lightweight and necessary for reproducibility.

## Measurement and reporting

- Separate raw result capture from processed summaries and figures.
- Keep scenario definitions in config or data files rather than hardcoding them into benchmark logic.
- When adding metrics, document their meaning and whether they are transport-level or application-level.
- Avoid changing benchmark methodology and algorithm logic in the same patch unless the coupling is unavoidable.
- Primary baseline comparisons should be Quinn NewReno, Cubic, and BBR.
- Multimedia gains must be reported together with fairness and coexistence results.

## Build and test

- Prefer `cargo check`, `cargo test`, and targeted crate-level commands before larger runs.
- Keep the documented validation path current in `README.md`; at minimum that includes local `cargo check` and the Linux VPS baseline plus shaped preview commands when those workflows change.
- Keep dependencies minimal and explicit at the workspace level.
- Do not introduce a Quinn fork, vendor tree, or custom patch dependency unless the project has already proven the public API is insufficient.

## Documentation

- Update `README.md` when the repo shape, build flow, or benchmark entry points change.
- Update `.github/copilot-instructions.md`, `docs/methodology.md`, and `TODO.md` when the validated experiment workflow or operational constraints change.
- If `gh` or `gcloud` become part of the expected operator workflow, document that explicitly rather than assuming future agents will infer it.
- Treat `README.md` as the operator/status guide, `docs/methodology.md` as the canonical scope and reproduction spec, and `docs/final-report.md` as the bounded result narrative.
- Keep focused appendices in `docs/` only when they add unique value, such as `docs/result-schema.md`, `docs/replay-semantics.md`, and `docs/live-demo.md`.
- Write short, concrete docs that make the experiment path obvious to a future reviewer.
