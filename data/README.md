# Data Layout

This directory holds local media inputs and derived replay artifacts for the Quinn experiments.

## Layout

- `raw/`: openly licensed source clips downloaded locally and ignored by Git
- `processed/segments/`: CMAF-style fragmented MP4 outputs created by `ffmpeg`
- `processed/manifests/`: replay metadata, semantic hints, and `ffprobe` sidecars used by the client and harness

## Workflow

1. Download source clips with `scripts/media/download_open_media.sh`.
2. Preprocess them with `scripts/media/preprocess_streams.sh`.
3. Point the client at the processed segments and manifests during replay testing.

The runtime sender should consume the derived artifacts and replay manifest, not parse arbitrary media containers directly.

The replay manifest is also where preprocessing now attaches lightweight semantic hints such as importance tiers, dependency depth hints, and freshness windows. The harness can use those hints when scoring replay units through `amc-core`.

Replay manifests are versioned inputs. The current builder writes `schema_version = 1`, and the client/harness preflight now validates that:

- the manifest file exists and is non-empty
- the init segment exists and is non-empty
- every referenced segment exists
- each segment size matches `size_bytes`
- the manifest is not older than any referenced payload file

The current heuristic rules for those semantic hints are documented in `docs/replay-semantics.md`.