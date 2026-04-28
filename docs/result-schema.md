# Sprint 1 Result Schema

This note captures the minimum JSON contract currently emitted by the local harness path for Sprint 1 reconciliation work.

The schema below is intentionally narrow. It only covers fields that are present in the current Rust producers and needed to reconcile raw and processed artifacts across Sprint 1 local runs.

## Scope

Current local harness execution writes three result shapes:

- raw transfer report under `results/raw/harness/*_report.json`
- processed per-run AMC analysis under `results/processed/harness/*_amc.json`
- processed suite summary under `results/processed/harness/*_summary.json`

The replay manifest referenced by processed outputs is an input artifact, not part of the result schema.

## Raw transfer report

File shape:

```json
{
  "summary": {
    "asset_name": "sintel_trailer",
    "baseline_controller": "cubic",
    "segments_received": 43,
    "media_segments_received": 42,
    "total_payload_bytes": 1750703,
    "useful_media_segments": 42,
    "late_media_segments": 0,
    "max_observed_lateness_ms": 0,
    "amc_runtime_samples": 0,
    "max_runtime_utility_score": null,
    "min_runtime_utility_score": null,
    "report_path": "C:\\Code\\quinn-amc\\results\\raw\\harness\\live_immediate_cubic_report.json"
  },
  "observations": [
    {
      "asset_name": "sintel_trailer",
      "mode": "live",
      "kind": "media",
      "sequence": 1,
      "start_time_ms": 0,
      "duration_ms": 1000,
      "deadline_ms": 1000,
      "client_send_elapsed_ms": 0,
      "server_receive_elapsed_ms": 2,
      "payload_len": 58139,
      "segment_path": "sintel_trailer_chunk_00001.m4s",
      "lateness_ms": -998,
      "useful": true,
      "runtime_utility": null
    }
  ]
}
```

Minimum contract:

- `summary.baseline_controller` is the provenance field for the QUIC baseline controller actually used on the connection.
- `summary.amc_runtime_samples` counts observations that carried runtime utility telemetry on the wire.
- `summary.max_runtime_utility_score` and `summary.min_runtime_utility_score` summarize the runtime utility score range observed by the server for that run.
- `summary.max_observed_lateness_ms` is a non-negative summary metric: it is the largest positive lateness seen among media segments, or `0` when no media segment arrived late.
- `summary.report_path` is written as an absolute path by the server.
- `observations[*].mode` distinguishes `vod` from `live` usefulness semantics.
- `observations[*].kind` distinguishes the init segment from media segments.
- `lateness_ms` is receiver-observed elapsed time minus sender-declared deadline.
- `useful` is `true` for init segments and for media that arrived by deadline.
- `observations[*].runtime_utility` is present when the sender used `amc_preview`; the current `cubic` and `bbr` baseline artifacts leave it `null`.

## Processed per-run AMC analysis

File shape:

```json
{
  "run_name": "live_immediate_cubic",
  "controller": "cubic",
  "asset_name": "sintel_trailer",
  "network_scenario": {
    "name": "local_loopback",
    "kind": "local"
  },
  "semantic_profile": {
    "startup_segments": 3,
    "live_freshness_window_ms": 1000
  },
  "aggregate": {
    "units_scored": 43,
    "media_units_scored": 42,
    "useful_media_units": 42,
    "zero_score_media_units": 0,
    "dependency_blocked_media_units": 0,
    "average_media_utility_score": 0.009,
    "useful_media_utility_sum": 0.39,
    "max_media_utility_score": 0.022,
    "min_media_utility_score": 0.005
  },
  "units": [
    {
      "sequence": 1,
      "segment_kind": "media",
      "traffic_class": "live",
      "importance": "critical",
      "dependency_depth": 0,
      "dependency_ready": true,
      "useful": true,
      "semantic_source": "replay_manifest",
      "payload_len": 58139,
      "segment_path": "sintel_trailer_chunk_00001.m4s",
      "delivery_deadline_ms": 1000,
      "freshness_window_ms": 1000,
      "queue_delay_ms": 0,
      "estimated_rtt_ms": 1,
      "utility_score": 0.0207
    }
  ]
}
```

Minimum contract:

- `controller` is copied from the harness run config and must match `summary.baseline_controller` in the raw report for the same run.
- `network_scenario` is embedded, not referenced by name only.
- `aggregate` is the stable comparison surface for Sprint 1 utility reconciliation.
- `units[*].semantic_source` shows whether scoring used replay-manifest hints or harness fallback values.

## Processed suite summary

File shape:

```json
{
  "suite_name": "local_live_immediate_baselines",
  "replay_manifest": "data/processed/manifests/sintel_trailer_replay.json",
  "network_scenarios": [
    {
      "name": "local_loopback",
      "kind": "local"
    }
  ],
  "runs": [
    {
      "name": "live_immediate_cubic",
      "controller": "cubic",
      "mode": "live",
      "pace": "immediate",
      "server": "127.0.0.1:5300",
      "report_path": "results/raw/harness/live_immediate_cubic_report.json",
      "network_scenario": {
        "name": "local_loopback",
        "kind": "local"
      },
      "amc_analysis_path": "results/processed/harness/live_immediate_cubic_amc.json",
      "amc_aggregate": {
        "units_scored": 43,
        "average_media_utility_score": 0.009
      },
      "summary": {
        "baseline_controller": "cubic",
        "amc_runtime_samples": 0,
        "max_runtime_utility_score": null,
        "min_runtime_utility_score": null,
        "report_path": "C:\\Code\\quinn-amc\\results\\raw\\harness\\live_immediate_cubic_report.json"
      }
    }
  ]
}
```

Minimum contract:

- `runs[*].controller` is the config-declared controller for the run.
- `runs[*].report_path` and `runs[*].amc_analysis_path` are workspace-relative paths.
- `runs[*].summary.report_path` remains the absolute raw-report path returned by the server.
- `runs[*].summary.amc_runtime_samples`, `runs[*].summary.max_runtime_utility_score`, and `runs[*].summary.min_runtime_utility_score` mirror the raw run-time telemetry summary when AMC runtime utility is present.
- The suite summary is the join point that links config intent, raw report location, and per-run processed analysis.

## Reconciliation rules

- For the same run, `runs[*].controller`, processed analysis `controller`, and raw `summary.baseline_controller` should agree exactly.
- Raw reports use an absolute `report_path`; processed summaries use relative `report_path` fields for the same artifact. This is expected in the current implementation.
- Windows can execute `local` scenarios, but `linux_tc_netem` scenarios only become runnable when the harness binary itself runs on Linux.