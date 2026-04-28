# Replay Semantics

## Purpose

This note documents the current heuristic rules used to attach lightweight semantic hints to replay segments during preprocessing.

These hints are not meant to be the final AMC semantic model. They are a reproducible bridge from offline packaged media artifacts to sender-visible utility inputs while the project is still trace-driven.

## Where semantics come from

The preprocessing step in `scripts/media/build_replay_manifest.py` augments each replay segment with a `semantic_hint` block and writes global defaults into `semantic_defaults`.

The harness then prefers those hint fields over its own fallback profile when it scores replay units through `amc-core`.

## Current heuristic rules

### Startup segments

- The first three media segments are labeled `startup`.
- Their `importance_hint` is `critical`.
- The first startup segment is marked `independent=true` with dependency depth `0`.
- Later startup segments keep dependency depth `1`.

Reasoning:

- Early delivery dominates startup delay and immediate playout continuity.
- The first segment after init should model a decoder-usable anchor unit.

### Size tiers

Segment sizes are compared against the lower and upper quartiles of the packaged segment-size distribution.

- Bottom quartile: `size_tier=small`
- Middle band: `size_tier=medium`
- Top quartile: `size_tier=large`

Reasoning:

- Size variation is a cheap proxy for burstiness and content complexity.
- It is available from offline packaging without introducing codec-specific parsing in the runtime path.

### Steady-state importance

- Large steady-state segments are labeled `burst`, marked `independent=true`, and upgraded to `importance_hint=high`.
- Other steady-state segments are labeled `steady` and use `importance_hint=normal`.

Reasoning:

- Large bursts often align with more structurally important or scene-changing content in chunked traces.
- Treating some larger units as independent creates a simple dependency pattern without full codec awareness.

### Freshness windows

- `small` segments get `freshness_window_ms = 1 x segment_duration`
- `medium` segments get `freshness_window_ms = 2 x segment_duration`
- `large` segments get `freshness_window_ms = 3 x segment_duration`

Reasoning:

- This is a deliberately simple way to vary usefulness decay without building a player.
- The harness later clamps these windows differently for `vod` and `live` runs.

## Fallback behavior

If a replay manifest does not provide semantic hints, the harness falls back to its configured semantic profile.

That fallback still controls:

- startup segment count
- VOD versus live steady-state importance
- dependency interval assumptions
- VOD versus live freshness bounds

## Limits

- These rules are heuristic and codec-agnostic by design.
- They should not be confused with true frame-type or decoder-dependency extraction.
- The next refinement should come from richer preprocessing artifacts rather than adding parser logic to the runtime sender.