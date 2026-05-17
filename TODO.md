# TODO

## Immediate blockers

- Keep the Phase 1 scope freeze intact: AMC v1 only, live-primary claim framing, mandatory fairness, VPS evidence plus local parity, and a single-run introspection demo.
- Keep the split canonical workflow documented consistently across README, methodology, VPS handoff notes, and copilot instructions until Phase 3 changes the runner model.
- Write the bounded interpretation for the fixed-preset VPS matrix and fairness guardrail artifacts that now define the final evidence families.

## Experiment completion

- Keep the fixed network presets stable across reruns and make any later preset change an explicit scope decision rather than quiet tuning.
- Phase 3: decide whether to extend the legacy VPS docker runner so coexistence runs can share the same host-veth path as the fixed-preset matrix, or retain the documented split workflow model.
- Decide whether the host-side runner should remain server-veth-only or move to explicitly symmetric shaping for later claims.
- Add explicit regeneration checks on real media-toolchain runs and verify regenerated manifests still work with the client and harness.

## AMC follow-up

- Keep the AMC v1 controller boundary explicit and honest about its current latest-sample, connection-wide runtime signal model.
- Do not widen `RuntimeUtilityState` beyond the latest sample until the AMC v1 artifact is finalized and frozen.
- Keep AMC v2 work explicitly out of the repository completion boundary unless a later scope change reopens it.
- Evaluate whether the first VPS matrix shows AMC value only in the shaped preview cell or also in the baseline cell before widening scope.

## Reporting and tooling

- Keep `README.md`, `docs/methodology.md`, and `.github/copilot-instructions.md` synchronized whenever the validated VPS path or `gcloud` operator commands change.
- Keep config classification explicit so final evidence, local parity, workflow-validation suites, and exploratory configs do not drift back together.
- Add a bounded result review that separates raw observations, tentative interpretation, and concrete AMC v2 follow-up work.
- Add the final result interpretation notes for the fixed-preset matrix and fairness guardrail suites once those frozen artifacts feed the report figures.
- Build the final figure package and report bundle around the frozen evidence families rather than widening the benchmark matrix.
- Finish the single-run `ratatui` live demo around one frozen raw report and document how to launch it.