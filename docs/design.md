# Design

## Design summary

AMC is treated as a semantic-aware transport policy with a congestion-control core.

The application layer should not send raw codec names or frame labels directly to the controller. Instead, it should provide a compact semantic description of each outgoing media unit.

The first implementation should use an offline preprocessing pipeline:

- `ffmpeg` generates CMAF-style fragmented MP4 outputs plus a DASH manifest
- a lightweight replay manifest is derived from the packaged output for runtime use and can carry segment-level semantic hints
- the Quinn sender consumes those artifacts directly during experiments

See `docs/replay-semantics.md` for the current documented heuristic mapping used during preprocessing.

## Application-to-transport interface

Each outgoing media unit should be annotated with sender-visible metadata:

- traffic class: VOD or live
- deadline: when the unit stops being useful
- importance: relative utility of successful delivery
- dependency depth: whether usefulness depends on earlier units
- freshness window: how quickly utility decays over time
- size: bytes scheduled for transmission

These fields are generic enough to work across traces, synthetic workloads, and multiple media encodings.

At runtime, the sender should reconstruct outgoing units from preprocessed segment or trace artifacts rather than opening and parsing arbitrary media containers on the fly.

Where possible, semantic hints should be attached during preprocessing and stored in the replay manifest so later evaluation does not depend only on ad hoc runtime sequence rules.

## Decision layers

Keep the implementation split into four layers:

1. Semantic input layer: application annotates outgoing units.
2. Utility layer: transport computes a utility score from those annotations.
3. Policy layer: sender prioritizes or deprioritizes work based on utility under congestion.
4. Congestion-control core: path-level send-rate and congestion-window logic.

This separation matters because it allows ablations and cleaner benchmarking.

## Initial policy expectations

The first iteration should support the following behaviors:

- higher-importance units are preferred during congestion
- units near expiry lose value quickly
- units whose dependencies are already stale should lose priority
- retransmission should be conservative when the re-delivered data is unlikely to remain useful

## Replay modes

Use the same preprocessed media source for two replay policies:

- VOD: buffered replay with bounded prefetch and looser usefulness decay
- live: low-lookahead replay with tight freshness windows and stronger deadline penalties

## Non-goals for first implementation

- codec-specific optimization beyond what can be expressed through generic metadata
- adaptive bitrate control loop integration
- datagram-based live transport in the primary path
- full in-process demuxing, decoding, or transcoding in the experiment sender