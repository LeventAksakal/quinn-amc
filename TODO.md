# TODO

## Immediate blockers

- Repair the init-segment size expectation so the live VPS controller matrix does not validate `sintel_trailer_init.mp4` against `0` bytes.
- Rerun `configs/harness/vps_live_realtime_controller_matrix.json` on the GCP VM and verify raw plus processed artifacts are produced for `new_reno`, `cubic`, `bbr`, and `amc_preview`.
- Keep runtime utility semantics, raw report provenance, and harness analysis aligned under the exact configs used for VPS evidence collection.

## Experiment completion

- Normalize VPS result ownership after `sudo` runs so `results/` artifacts stop landing as `root` on the VM.
- Reduce the GCP VPS first-run build cost by enabling `buildx` or otherwise caching the Docker build path more aggressively.
- Replace the preview-only shaped VPS configs with a fixed preset matrix that spans `vod` and `live` across `new_reno`, `cubic`, `bbr`, and `amc_preview`.
- Define and validate fixed network presets such as `wired_clean`, `wifi_moderate`, `wifi_unstable`, `lte_moderate`, and `lte_constrained` using explicit `tc` parameters.
- Add fairness and coexistence runs once the AMC path is ready to compare against Cubic, NewReno, and BBR under the same VPS workflow.
- Decide whether the host-side runner should remain server-veth-only or move to explicitly symmetric shaping for later claims.
- Add explicit regeneration checks on real media-toolchain runs and verify regenerated manifests still work with the client and harness.

## AMC follow-up

- Keep the AMC v1 controller boundary explicit and honest about its current latest-sample, connection-wide runtime signal model.
- Widen `RuntimeUtilityState` beyond the latest sample into a richer sender-state snapshot for AMC v2.
- Add backlog composition or urgency summaries so AMC decisions are not driven only by the most recent utility update.
- Evaluate whether the first VPS matrix shows AMC value only in the shaped preview cell or also in the baseline cell before widening scope.

## Reporting and tooling

- Keep `README.md`, `docs/methodology.md`, and `.github/copilot-instructions.md` synchronized whenever the validated VPS path or `gcloud` operator commands change.
- Add a bounded result review that separates raw observations, tentative interpretation, and concrete AMC v2 follow-up work.
- Add a short result interpretation note once processed VPS summaries start feeding the report figures.
- Deepen figure-ready export and reporting support beyond the current JSON-only artifact set as the comparison workflow solidifies.
- Add a plotting pipeline that turns harness summaries and comparison exports into figure-ready PNG or SVG outputs for the report.
- Add a `ratatui`-based live demo once the harness outputs and comparison schema are stable enough to drive a terminal presentation layer.