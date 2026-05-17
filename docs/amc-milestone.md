# AMC Milestone

## Purpose

This note records the Phase 4 controller-completion decision for the repository.

The goal is to make the AMC boundary explicit before Phase 5 freezes artifacts, figures, and report packaging.

## Phase 4 Decision

Repository completion stops at AMC v1.

For this repository, AMC v1 means:

- the sender can score outgoing media units from `traffic class`, `deadline`, `importance`, `dependency depth`, `freshness window`, and size
- the live sender path can reorder ready segments by that score before transmission
- the congestion controller itself sees only the latest connection-wide `UtilitySignal`
- Quinn is not being asked to provide per-packet semantic annotations, per-stream congestion state, or a history-aware runtime-state model

AMC v2 remains explicit future work outside the repository completion boundary.

## Controller-Completion Criteria

AMC is considered complete for this repository when all of the following are true:

1. Controller identity is auditable end to end from config to client invocation to raw report to processed output.
2. The frozen VPS live matrix shows bounded value from semantic-aware behavior under at least the harder constrained presets, even if that value does not exceed the strongest baseline on every cell.
3. The required fairness guardrail suite shows acceptable coexistence with a BBR competitor in throughput share and Jain fairness.
4. The VOD path is reported honestly as supporting evidence rather than overclaimed as an AMC win condition.
5. The repository does not require widening `RuntimeUtilityState` or adding AMC v2 state expansion to count as complete.

These criteria deliberately describe a bounded milestone, not a universal claim that AMC outperforms every baseline in every scenario.

## Frozen Evidence Reading

The current frozen VPS evidence supports this narrower interpretation:

- BBR remains the strongest overall baseline in the fixed single-flow matrix, especially on live age-of-information and deadline stability under moderate and clean presets.
- AMC v1 shows bounded live value on the hardest constrained presets relative to the loss-based baselines `new_reno` and `cubic`, but it does not reach BBR parity.
- AMC v1 is fairness-safe in the required BBR guardrail suite at the throughput-sharing level, but that does not mean it matches BBR on freshness-sensitive live outcomes while competing.
- VOD remains supporting evidence only. AMC v1 is not a startup-delay winner and should not be framed that way.

## Key Evidence Snapshot

### Live single-flow matrix

Under `wifi_unstable` live replay:

- `amc_preview` and `bbr` both avoid deadline misses, while `new_reno` and `cubic` still miss about `2.38%`
- `amc_preview` useful live utility sum is effectively tied with `bbr` (`0.230367` vs `0.230444`) and above `new_reno` / `cubic`
- `amc_preview` average age of information remains far above `bbr` (`135.67 ms` vs `17.83 ms`)

Under `lte_constrained` live replay:

- `amc_preview` cuts deadline miss rate relative to `new_reno` and `cubic` (`4.76%` vs `9.52%`)
- `amc_preview` useful live utility sum exceeds `new_reno` and `cubic` (`0.201769` vs `0.191086` and `0.187354`)
- `bbr` still leads the cell on utility sum, zero deadline misses, and age of information

Under `wired_clean`, `wifi_moderate`, and `lte_moderate` live replay:

- AMC v1 stays close to the loss-based baselines on throughput and aggregate utility
- BBR remains clearly stronger on freshness-sensitive delivery metrics
- those cells should be interpreted as bounded or neutral evidence for AMC v1 rather than broad superiority

### VOD supporting evidence

- VOD throughput and aggregate utility remain close across controllers in the frozen matrix
- AMC v1 does not improve startup delay consistently
- the hardest VOD preset, `lte_constrained`, is materially worse for AMC startup delay than BBR (`3509 ms` vs `2023 ms`)

### Fairness guardrail

Against the required BBR competitor suite:

- foreground throughput share stays effectively even for every controller, including AMC v1
- Jain fairness remains effectively `1.0`
- AMC v1 therefore passes the required throughput-fairness guardrail, even though its foreground live freshness metrics still trail BBR in the same constrained settings

## Repository Claim After Phase 4

The repository claim after Phase 4 is:

- AMC v1 demonstrates that sender-visible semantic signals can improve the hardest live constrained cases relative to standard loss-based Quinn baselines without breaking the required BBR fairness guardrail.
- AMC v1 does not justify a claim of broad dominance over BBR across the fixed matrix.
- AMC v1 does not justify a claim of VOD startup superiority.
- Phase 5 may freeze the evidence set using this bounded claim without reopening controller design.

## Explicitly Deferred To AMC v2

- widening `RuntimeUtilityState` beyond the latest sample
- feeding backlog composition, class-local urgency, or history into the controller
- per-stream or per-packet semantic isolation inside Quinn
- broader fairness coverage beyond the current BBR guardrail family
- stronger live freshness performance claims that would require more than the current v1 mechanism