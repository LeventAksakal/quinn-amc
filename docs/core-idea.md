# Core Idea

## Thesis

The project does not claim to build a BBRv2 replacement.

The intended contribution is a semantic-aware transport policy with a congestion-control core for user-space QUIC. Quinn runs in user space, so the application can expose multimedia semantics to the sender before transport-policy decisions are made.

## Problem statement

Generic congestion controllers optimize path usage with little knowledge of application utility. Multimedia applications care about a different outcome: which bytes arrive in time and remain useful for playout or decode.

This mismatch creates space for a sender that can distinguish between high-utility and low-utility traffic under the same network conditions.

## Intended contribution

The design should let the application annotate outgoing units with a compact, codec-agnostic set of semantics:

- deadline
- importance
- dependency depth
- freshness window

The transport policy then uses those signals to guide congestion response and recovery decisions.

The runtime system should consume preprocessed traces or stream segments derived offline with `ffmpeg` and `ffprobe`, not a full in-process media stack.

## Scope boundaries

- Main transport substrate: QUIC streams
- Main baselines: Quinn NewReno, Cubic, and BBR
- Main traffic classes: VOD and live
- Main claim: improved multimedia utility with acceptable fairness

Out of primary scope:

- claiming to replace BBRv2
- relying on QUIC datagrams for the core result
- making broad TCP comparison claims as the main evaluation axis
- building a full player, encoder, or topology emulator as part of the main artifact

## Research framing

The core question is whether application-aware sender decisions can improve useful multimedia delivery under congestion while coexisting reasonably with standard Quinn congestion controllers.