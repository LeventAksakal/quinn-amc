# Design

## Design summary

AMC is treated as a semantic-aware transport policy with a congestion-control core.

The application layer should not send raw codec names or frame labels directly to the controller. Instead, it should provide a compact semantic description of each outgoing media unit.

## Application-to-transport interface

Each outgoing media unit should be annotated with sender-visible metadata:

- traffic class: VOD or live
- deadline: when the unit stops being useful
- importance: relative utility of successful delivery
- dependency depth: whether usefulness depends on earlier units
- freshness window: how quickly utility decays over time
- size: bytes scheduled for transmission

These fields are generic enough to work across traces, synthetic workloads, and multiple media encodings.

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

## Non-goals for first implementation

- codec-specific optimization beyond what can be expressed through generic metadata
- adaptive bitrate control loop integration
- datagram-based live transport in the primary path