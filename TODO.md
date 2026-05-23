# TODO

Repository completion is reached at the AMC v1 boundary. This file now tracks only future work and integrity guards.

## Integrity guards

- Keep the AMC v1 scope freeze intact: live-primary claim framing, mandatory fairness, VPS evidence as the canonical report surface, and the single-run introspection demo.
- Keep the split canonical VPS workflow explicit until a later change truly adds multi-client docker-runner coexistence support.
- Keep the canonical evidence boundary intact whenever figures, reports, or support reruns are revisited.
- Keep `README.md`, `docs/methodology.md`, and `.github/copilot-instructions.md` synchronized whenever the validated operator workflow changes.
- Keep config classification explicit so final evidence, local parity, workflow-validation suites, and exploratory configs do not drift back together.

## Future work outside repository completion

- decide whether the host-side runner should remain server-veth-only or move to explicitly symmetric shaping for later claims
- explore AMC v2 controller-state expansion beyond the current latest-sample, connection-wide runtime signal model
- improve AMC live tail-latency behavior on the hardest constrained presets without overstating parity with BBR
- reduce AMC VOD startup delay so VOD becomes more than bounded supporting evidence
- extend coexistence coverage beyond the current host-run fairness guardrail and close the docker-runner multi-client gap
- revisit additional transport axes such as QUIC datagrams only if a later scope change promotes them into the main claim