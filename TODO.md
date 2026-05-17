# TODO

## Immediate blockers

- Keep the Phase 1 scope freeze intact: AMC v1 only, live-primary claim framing, mandatory fairness, VPS evidence plus local parity, and a single-run introspection demo.
- Keep the split canonical workflow documented consistently across README, methodology, VPS handoff notes, and copilot instructions until Phase 3 changes the runner model.
- Carry the bounded Phase 4 AMC interpretation into the frozen evidence package, figures, and report text without widening the controller claim.

## Experiment completion

- Keep the fixed network presets stable across reruns and make any later preset change an explicit scope decision rather than quiet tuning.
- Keep the current split VPS workflow model explicit until a later phase truly adds multi-client docker-runner support; do not blur the compose matrix path and the host fairness path back together.
- Decide whether the host-side runner should remain server-veth-only or move to explicitly symmetric shaping for later claims.
- Run an explicit end-to-end regeneration check from `scripts/media/preprocess_streams.sh` through a local harness suite on both packaged assets before calling the replay pipeline frozen.

## AMC follow-up

- Keep the AMC v1 controller boundary explicit and honest about its current latest-sample, connection-wide runtime signal model.
- Do not widen `RuntimeUtilityState` beyond the latest sample until the AMC v1 artifact is finalized and frozen.
- Keep AMC v2 work explicitly out of the repository completion boundary unless a later scope change reopens it.
- If later frozen reruns materially change the constrained-live picture, treat that as evidence-review work for Phase 5 rather than as a trigger to quietly widen the controller scope.

## Reporting and tooling

- Keep `README.md`, `docs/methodology.md`, and `.github/copilot-instructions.md` synchronized whenever the validated VPS path or `gcloud` operator commands change.
- Keep config classification explicit so final evidence, local parity, workflow-validation suites, and exploratory configs do not drift back together.
- Add the final result interpretation notes for the fixed-preset matrix and fairness guardrail suites once those frozen artifacts feed the report figures.
- Build the final figure package and report bundle around the frozen evidence families rather than widening the benchmark matrix.
- Finish the single-run `ratatui` live demo around one frozen raw report and document how to launch it.