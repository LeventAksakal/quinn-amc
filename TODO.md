# TODO

## Immediate blockers

- Review the generated `vps_live_realtime_controller_matrix` raw and processed artifacts and write down the bounded interpretation before widening the matrix scope.
- Keep runtime utility semantics, raw report provenance, harness analysis, and figure outputs aligned under the exact configs used for VPS evidence collection.
- Validate the fixed preset matrix end to end on the GCP VM with `configs/harness/vps_fixed_preset_controller_matrix.json` once the first live matrix review is captured.

## Experiment completion

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
- Add a `ratatui`-based live demo once the harness outputs and comparison schema are stable enough to drive a terminal presentation layer.