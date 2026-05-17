#!/usr/bin/env bash
set -euo pipefail

CONFIG_PATH="${1:-configs/harness/vps_demo_vod_live.json}"
WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIG_ABS="$WORKSPACE_ROOT/$CONFIG_PATH"
COMPOSE_FILE="$WORKSPACE_ROOT/compose.yaml"
ACTIVE_TC_INTERFACE=""
CURRENT_RUN_NAME=""
RESULT_OWNER_USER="${SUDO_USER:-$(id -un)}"
RESULT_OWNER_GROUP="$(id -gn "$RESULT_OWNER_USER" 2>/dev/null || id -gn)"
RESULT_OWNER_SPEC="$RESULT_OWNER_USER:$RESULT_OWNER_GROUP"
RUNNER_LOG_DIR="$WORKSPACE_ROOT/results/raw/harness/runner"
BUILD_STAMP_PATH="$WORKSPACE_ROOT/results/.vps_suite_build_stamp"
CLIENT_TIMEOUT_SECONDS="${CLIENT_TIMEOUT_SECONDS:-600}"
HARNESS_ANALYZE_TIMEOUT_SECONDS="${HARNESS_ANALYZE_TIMEOUT_SECONDS:-600}"
SERVER_LOG_TAIL_LINES="${SERVER_LOG_TAIL_LINES:-200}"

log() {
  printf '[run_linux_vps_suite] %s\n' "$*"
}

fail() {
  log "ERROR: $*" >&2
  exit 1
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

require_command docker
require_command jq
require_command tc
require_command nsenter
require_command ip
require_command timeout

[[ -f "$CONFIG_ABS" ]] || fail "config not found: $CONFIG_ABS"
[[ -f "$COMPOSE_FILE" ]] || fail "compose file not found: $COMPOSE_FILE"

validate_runner_supported_config() {
  if jq -e '.runs[] | select(.coexistence != null)' "$CONFIG_ABS" >/dev/null 2>&1; then
    fail "config $CONFIG_PATH includes coexistence flows, but run_linux_vps_suite.sh launches exactly one demo-client container per run. Use the host harness path with configs/harness/vps_host_live_coexistence_bbr_guardrail.json instead."
  fi
}

validate_replay_manifest_assets() {
  local manifest_rel_path manifest_abs_path asset_name init_segment asset_dir segment_count
  manifest_rel_path="$(jq -r '.replay_manifest' "$CONFIG_ABS")"
  [[ -n "$manifest_rel_path" && "$manifest_rel_path" != "null" ]] || fail "config $CONFIG_PATH is missing replay_manifest"
  manifest_abs_path="$WORKSPACE_ROOT/$manifest_rel_path"
  [[ -f "$manifest_abs_path" ]] || fail "replay manifest not found: $manifest_abs_path"
  [[ -s "$manifest_abs_path" ]] || fail "replay manifest is empty: $manifest_abs_path"

  asset_name="$(jq -r '.asset_name // empty' "$manifest_abs_path")"
  init_segment="$(jq -r '.init_segment // empty' "$manifest_abs_path")"
  [[ -n "$asset_name" ]] || fail "replay manifest $manifest_abs_path is missing asset_name"
  [[ -n "$init_segment" ]] || fail "replay manifest $manifest_abs_path is missing init_segment"

  asset_dir="$WORKSPACE_ROOT/data/processed/segments/$asset_name"
  [[ -d "$asset_dir" ]] || fail "segment asset directory not found: $asset_dir"
  [[ -f "$asset_dir/$init_segment" ]] || fail "init segment not found: $asset_dir/$init_segment"
  [[ -s "$asset_dir/$init_segment" ]] || fail "init segment is empty: $asset_dir/$init_segment"

  segment_count=0
  while IFS=$'\t' read -r relative_path size_bytes; do
    [[ -n "$relative_path" ]] || continue
    local segment_path actual_size
    segment_path="$asset_dir/$relative_path"
    [[ -f "$segment_path" ]] || fail "segment payload not found: $segment_path"
    [[ -s "$segment_path" ]] || fail "segment payload is empty: $segment_path"
    actual_size="$(stat -c %s "$segment_path")"
    [[ "$actual_size" == "$size_bytes" ]] || fail "segment payload size mismatch for $segment_path: manifest says $size_bytes, file has $actual_size"
    if [[ "$manifest_abs_path" -ot "$segment_path" ]]; then
      fail "replay manifest $manifest_abs_path is older than asset payload $segment_path; rerun scripts/media/preprocess_streams.sh before executing the suite"
    fi
    segment_count=$((segment_count + 1))
  done < <(jq -r '.segments[] | "\(.relative_path)\t\(.size_bytes)"' "$manifest_abs_path")

  [[ "$segment_count" -gt 0 ]] || fail "replay manifest $manifest_abs_path does not reference any media segments"
  log "replay manifest validated: asset=$asset_name segments=$segment_count manifest=$manifest_rel_path"
}

export DOCKER_BUILDKIT="${DOCKER_BUILDKIT:-1}"
export COMPOSE_DOCKER_CLI_BUILD="${COMPOSE_DOCKER_CLI_BUILD:-1}"

validate_runner_supported_config
validate_replay_manifest_assets

can_sudo_non_interactive() {
  sudo -n true >/dev/null 2>&1
}

run_as_root() {
  if [[ "$EUID" -eq 0 ]]; then
    "$@"
    return 0
  fi

  if can_sudo_non_interactive; then
    sudo "$@"
    return 0
  fi

  fail "root privileges required to run: $*"
}

ensure_directory_writable() {
  local dir_path="$1"

  mkdir -p "$dir_path" >/dev/null 2>&1 || true
  if [[ -d "$dir_path" && -w "$dir_path" ]]; then
    return 0
  fi

  if can_sudo_non_interactive; then
    run_as_root mkdir -p "$dir_path"
    run_as_root chown -R "$RESULT_OWNER_SPEC" "$dir_path"
  fi

  [[ -d "$dir_path" && -w "$dir_path" ]] || fail "directory is not writable: $dir_path"
}

normalize_result_ownership() {
  local path="$1"

  [[ -e "$path" ]] || return 0

  if [[ "$EUID" -ne 0 ]] && ! can_sudo_non_interactive; then
    return 0
  fi

  run_as_root chown -R "$RESULT_OWNER_SPEC" "$path"
}

remove_output_file() {
  local file_path="$1"

  if [[ ! -e "$file_path" ]]; then
    return 0
  fi

  if rm -f "$file_path" >/dev/null 2>&1; then
    return 0
  fi

  if [[ "$EUID" -eq 0 ]] || can_sudo_non_interactive; then
    run_as_root rm -f "$file_path"
    return 0
  fi

  fail "failed to remove output file: $file_path"
}

prepare_output_paths() {
  ensure_directory_writable "$WORKSPACE_ROOT/results"
  ensure_directory_writable "$WORKSPACE_ROOT/results/raw"
  ensure_directory_writable "$WORKSPACE_ROOT/results/raw/harness"
  ensure_directory_writable "$RUNNER_LOG_DIR"
  ensure_directory_writable "$WORKSPACE_ROOT/results/processed"
  ensure_directory_writable "$WORKSPACE_ROOT/results/processed/harness"
  normalize_result_ownership "$WORKSPACE_ROOT/results"
}

latest_build_input_epoch() {
  local latest=0
  local candidate_paths=(
    "$WORKSPACE_ROOT/Cargo.toml"
    "$WORKSPACE_ROOT/Cargo.lock"
    "$WORKSPACE_ROOT/rust-toolchain.toml"
    "$WORKSPACE_ROOT/compose.yaml"
    "$WORKSPACE_ROOT/docker"
    "$WORKSPACE_ROOT/crates"
    "$WORKSPACE_ROOT/configs"
  )
  local candidate epoch

  for candidate in "${candidate_paths[@]}"; do
    if [[ -f "$candidate" ]]; then
      epoch="$(stat -c %Y "$candidate")"
    elif [[ -d "$candidate" ]]; then
      epoch="$(find "$candidate" -type f -printf '%T@\n' | sort -nr | head -n 1 | cut -d. -f1)"
    else
      continue
    fi

    [[ -n "$epoch" ]] || continue
    if (( epoch > latest )); then
      latest="$epoch"
    fi
  done

  printf '%s\n' "$latest"
}

suite_images_present() {
  docker image inspect \
    quinn-amc/demo-server:distroless \
    quinn-amc/demo-client:distroless \
    quinn-amc/harness:distroless >/dev/null 2>&1
}

build_services_if_needed() {
  local latest_epoch stamp_epoch=0
  latest_epoch="$(latest_build_input_epoch)"

  if [[ -f "$BUILD_STAMP_PATH" ]]; then
    stamp_epoch="$(stat -c %Y "$BUILD_STAMP_PATH")"
  fi

  if suite_images_present && (( stamp_epoch >= latest_epoch )); then
    log "skipping compose build: cached images are newer than tracked build inputs"
    return 0
  fi

  log "building compose services: demo-server demo-client harness"
  docker compose -f "$COMPOSE_FILE" --profile demo-server --profile demo-client --profile harness build demo-server demo-client harness
  touch "$BUILD_STAMP_PATH"
}

run_log_path() {
  local suffix="$1"
  printf '%s/%s_%s.log\n' "$RUNNER_LOG_DIR" "$CURRENT_RUN_NAME" "$suffix"
}

capture_tc_snapshot() {
  local interface="$1"
  local output_path="$2"

  {
    printf 'captured_at=%s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    printf 'interface=%s\n' "$interface"
    echo '--- qdisc ---'
    tc qdisc show dev "$interface"
    echo '--- qdisc stats ---'
    tc -s qdisc show dev "$interface"
  } >"$output_path" 2>&1 || true
}

capture_demo_server_logs() {
  local output_path="$1"
  docker compose -f "$COMPOSE_FILE" logs --no-color --tail "$SERVER_LOG_TAIL_LINES" demo-server >"$output_path" 2>&1 || true
}

log_file_tail() {
  local label="$1"
  local file_path="$2"

  [[ -f "$file_path" ]] || return 0

  log "$label tail: $file_path"
  tail -n 40 "$file_path" || true
}

run_logged_with_timeout() {
  local timeout_seconds="$1"
  local output_path="$2"
  shift 2

  : >"$output_path"

  set +e
  timeout --signal=TERM --kill-after=10s "${timeout_seconds}s" "$@" > >(tee "$output_path") 2>&1
  local status=$?
  set -e

  return "$status"
}

cleanup_tc_interface() {
  local interface="${ACTIVE_TC_INTERFACE:-}"
  if [[ -n "$interface" ]]; then
    log "cleanup: clearing tc qdisc on interface $interface"
    tc qdisc del dev "$interface" root >/dev/null 2>&1 || log "cleanup: no tc qdisc removed from $interface"
    ACTIVE_TC_INTERFACE=""
  fi
}

cleanup_demo_server() {
  log "cleanup: removing demo-server container"
  docker compose -f "$COMPOSE_FILE" rm -sf demo-server >/dev/null 2>&1 || log "cleanup: demo-server removal skipped"
}

cleanup() {
  local exit_code="$1"

  cleanup_tc_interface
  cleanup_demo_server
  normalize_result_ownership "$WORKSPACE_ROOT/results"

  if [[ "$exit_code" -eq 0 ]]; then
    log "cleanup: complete"
  else
    local run_label="${CURRENT_RUN_NAME:-suite setup}"
    log "cleanup: complete after failure during $run_label"
  fi
}

trap 'cleanup "$?"' EXIT

get_run_field() {
  local run_name="$1"
  local field="$2"
  jq -r --arg run_name "$run_name" --arg field "$field" '
    .runs[] | select(.name == $run_name) | .[$field]
  ' "$CONFIG_ABS"
}

get_scenario_json() {
  local scenario_name="$1"
  jq -c --arg scenario_name "$scenario_name" '
    .network_scenarios[] | select(.name == $scenario_name)
  ' "$CONFIG_ABS"
}

wait_for_file() {
  local file_path="$1"
  local attempts=0
  while [[ ! -f "$file_path" ]]; do
    attempts=$((attempts + 1))
    if [[ "$attempts" -gt 100 ]]; then
      echo "timed out waiting for $file_path" >&2
      exit 1
    fi
    sleep 0.2
  done
}

wait_for_container_running() {
  local service_name="$1"
  local attempts=0
  local container_id=""
  local running_state=""

  while :; do
    container_id="$(docker compose -f "$COMPOSE_FILE" ps -q "$service_name")"
    if [[ -n "$container_id" ]]; then
      running_state="$(docker inspect -f '{{.State.Running}}' "$container_id" 2>/dev/null || true)"
      if [[ "$running_state" == "true" ]]; then
        printf '%s\n' "$container_id"
        return 0
      fi
    fi

    attempts=$((attempts + 1))
    if [[ "$attempts" -gt 100 ]]; then
      fail "timed out waiting for $service_name container to become running"
    fi
    sleep 0.2
  done
}

get_container_ip() {
  local container_id="$1"
  local container_ip

  container_ip="$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$container_id")"
  [[ -n "$container_ip" ]] || fail "failed to resolve container IP for $container_id"
  printf '%s\n' "$container_ip"
}

get_container_host_veth() {
  local container_id="$1"
  local pid
  local container_interface_line
  local container_interface
  local iflink
  local host_interface

  pid="$(docker inspect -f '{{.State.Pid}}' "$container_id")"
  [[ -n "$pid" && "$pid" != "0" ]] || fail "container $container_id is not running with a valid PID"

  container_interface_line="$(nsenter -t "$pid" -n ip -o link | awk -F': ' '$2 !~ /^lo/ { print $2; exit }')"
  [[ -n "$container_interface_line" ]] || fail "failed to resolve container interface for $container_id"

  container_interface="${container_interface_line%%@*}"
  iflink="$(sed -n 's/.*@if\([0-9]\+\).*/\1/p' <<<"$container_interface_line")"
  [[ -n "$iflink" ]] || fail "failed to resolve host iflink from container interface $container_interface_line for $container_id"

  host_interface="$(ip -o link | awk -F': ' -v idx="$iflink" '$1 == idx { print $2; exit }')"
  host_interface="${host_interface%%@*}"
  [[ -n "$host_interface" ]] || fail "failed to resolve host veth for iflink $iflink from container $container_id"

  log "resolved host veth: container=$container_id pid=$pid container_interface=$container_interface iflink=$iflink interface=$host_interface" >&2
  printf '%s\n' "$host_interface"
}

tc_device_exists() {
  local interface="$1"
  ip link show dev "$interface" >/dev/null 2>&1
}

apply_tc_from_scenario() {
  local interface="$1"
  local scenario_json="$2"
  local enabled
  local scenario_name

  scenario_name="$(jq -r '.name' <<<"$scenario_json")"
  enabled="$(jq -r '.tc_netem_enabled' <<<"$scenario_json")"
  if [[ "$enabled" != "true" ]]; then
    log "tc disabled for scenario $scenario_name"
    return 0
  fi

  local rtt_ms loss_percent bandwidth_mbps delay_jitter_ms limit_packets rate_burst_kbit rate_latency_ms
  rtt_ms="$(jq -r '.rtt_ms // empty' <<<"$scenario_json")"
  loss_percent="$(jq -r '.loss_percent // empty' <<<"$scenario_json")"
  bandwidth_mbps="$(jq -r '.bandwidth_mbps // empty' <<<"$scenario_json")"
  delay_jitter_ms="$(jq -r '.tc_netem.delay_jitter_ms // empty' <<<"$scenario_json")"
  limit_packets="$(jq -r '.tc_netem.limit_packets // empty' <<<"$scenario_json")"
  rate_burst_kbit="$(jq -r '.tc_netem.rate_burst_kbit // empty' <<<"$scenario_json")"
  rate_latency_ms="$(jq -r '.tc_netem.rate_latency_ms // empty' <<<"$scenario_json")"

  tc_device_exists "$interface" || fail "tc target interface does not exist: $interface"

  local args=(qdisc replace dev "$interface" root handle 1: netem)
  if [[ -n "$rtt_ms" ]]; then
    args+=(delay "${rtt_ms}ms")
    if [[ -n "$delay_jitter_ms" ]]; then
      args+=("${delay_jitter_ms}ms")
    fi
  fi
  if [[ -n "$loss_percent" && "$loss_percent" != "0" && "$loss_percent" != "0.0" ]]; then
    args+=(loss "${loss_percent}%")
  fi
  if [[ -n "$limit_packets" ]]; then
    args+=(limit "$limit_packets")
  fi

  log "applying tc netem: scenario=$scenario_name interface=$interface rtt_ms=${rtt_ms:-none} jitter_ms=${delay_jitter_ms:-none} loss_percent=${loss_percent:-0} limit_packets=${limit_packets:-default}"
  tc "${args[@]}"

  if [[ -n "$bandwidth_mbps" ]]; then
    local burst latency
    burst="${rate_burst_kbit:-256}"
    latency="${rate_latency_ms:-50}"
    log "applying tc tbf: scenario=$scenario_name interface=$interface bandwidth_mbps=$bandwidth_mbps burst_kbit=$burst latency_ms=$latency"
    tc qdisc replace dev "$interface" parent 1:1 handle 10: tbf rate "${bandwidth_mbps}mbit" burst "${burst}kbit" latency "${latency}ms"
  else
    log "tc tbf skipped: scenario=$scenario_name interface=$interface bandwidth_mbps=none"
  fi

  ACTIVE_TC_INTERFACE="$interface"
  log "tc active: scenario=$scenario_name interface=$interface"
}

RUN_NAMES="$(jq -r '.runs[].name' "$CONFIG_ABS")"
SERVER_CERT_HOST_PATH="$WORKSPACE_ROOT/results/demo-cert.der"
REPLAY_MANIFEST_IN_CONTAINER="/workspace/$(jq -r '.replay_manifest' "$CONFIG_ABS")"
HARNESS_CONFIG_IN_CONTAINER="/workspace/$CONFIG_PATH"

prepare_output_paths

build_services_if_needed

for run_name in $RUN_NAMES; do
  CURRENT_RUN_NAME="$run_name"
  scenario_name="$(get_run_field "$run_name" network_scenario)"
  controller="$(get_run_field "$run_name" controller)"
  mode="$(get_run_field "$run_name" mode)"
  pace="$(get_run_field "$run_name" pace)"
  vod_deadline_slack_ms="$(get_run_field "$run_name" vod_deadline_slack_ms)"
  report_in_container="/workspace/results/raw/harness/${run_name}_report.json"
  report_on_host="$WORKSPACE_ROOT/results/raw/harness/${run_name}_report.json"
  client_log_path="$(run_log_path client)"
  server_log_path="$(run_log_path server)"
  tc_log_path="$(run_log_path tc)"
  timing_log_path="$(run_log_path timing)"
  scenario_json="$(get_scenario_json "$scenario_name")"
  [[ -n "$scenario_json" ]] || fail "run $run_name references unknown scenario $scenario_name"

  run_started_epoch="$(date +%s)"

  log "starting run: name=$run_name scenario=$scenario_name controller=$controller mode=$mode pace=$pace vod_deadline_slack_ms=${vod_deadline_slack_ms:-unset}"

  cleanup_tc_interface
  cleanup_demo_server

  prepare_output_paths
  remove_output_file "$report_on_host"
  remove_output_file "$SERVER_CERT_HOST_PATH"
  remove_output_file "$client_log_path"
  remove_output_file "$server_log_path"
  remove_output_file "$tc_log_path"
  remove_output_file "$timing_log_path"
  log "run setup: cleared prior report and certificate outputs for $run_name"

  server_start_epoch="$(date +%s)"
  DEMO_SERVER_REPORT_OUT="$report_in_container" \
  DEMO_SERVER_CERT_OUT="/workspace/results/demo-cert.der" \
  DEMO_SERVER_PORT=5001 \
  docker compose -f "$COMPOSE_FILE" --profile demo-server up -d demo-server

  server_container_id="$(wait_for_container_running demo-server)"
  log "demo-server running: container=$server_container_id"
  wait_for_file "$SERVER_CERT_HOST_PATH"
  log "server certificate ready: $SERVER_CERT_HOST_PATH"
  server_container_ip="$(get_container_ip "$server_container_id")"
  log "resolved server container IP: container=$server_container_id ip=$server_container_ip"
  server_host_veth="$(get_container_host_veth "$server_container_id")"
  apply_tc_from_scenario "$server_host_veth" "$scenario_json"
  capture_tc_snapshot "$server_host_veth" "$tc_log_path"
  server_ready_epoch="$(date +%s)"

  client_start_epoch="$(date +%s)"
  client_status=0
  run_logged_with_timeout \
    "$CLIENT_TIMEOUT_SECONDS" \
    "$client_log_path" \
    env \
    DEMO_CLIENT_SERVER="$server_container_ip:5001" \
    DEMO_CLIENT_SERVER_NAME="localhost" \
    DEMO_CLIENT_CERT="/workspace/results/demo-cert.der" \
    DEMO_CLIENT_REPLAY_MANIFEST="$REPLAY_MANIFEST_IN_CONTAINER" \
    DEMO_CLIENT_PACE="$pace" \
    DEMO_CLIENT_MODE="$mode" \
    DEMO_CLIENT_CONTROLLER="$controller" \
    DEMO_CLIENT_VOD_DEADLINE_SLACK_MS="$vod_deadline_slack_ms" \
    docker compose -f "$COMPOSE_FILE" --profile demo-server --profile demo-client run --rm --no-deps demo-client || client_status=$?
  client_finished_epoch="$(date +%s)"

  capture_demo_server_logs "$server_log_path"
  capture_tc_snapshot "$server_host_veth" "$tc_log_path"

  {
    printf 'run_name=%s\n' "$run_name"
    printf 'scenario=%s\n' "$scenario_name"
    printf 'controller=%s\n' "$controller"
    printf 'mode=%s\n' "$mode"
    printf 'pace=%s\n' "$pace"
    printf 'client_exit_code=%s\n' "$client_status"
    printf 'server_startup_seconds=%s\n' "$((server_ready_epoch - server_start_epoch))"
    printf 'client_runtime_seconds=%s\n' "$((client_finished_epoch - client_start_epoch))"
    printf 'total_runtime_seconds=%s\n' "$((client_finished_epoch - run_started_epoch))"
    printf 'client_log=%s\n' "$client_log_path"
    printf 'server_log=%s\n' "$server_log_path"
    printf 'tc_log=%s\n' "$tc_log_path"
  } >"$timing_log_path"

  if [[ "$client_status" -ne 0 ]]; then
    log_file_tail "client log" "$client_log_path"
    log_file_tail "server log" "$server_log_path"
    if [[ "$client_status" -eq 124 || "$client_status" -eq 137 ]]; then
      fail "client timed out for $run_name after ${CLIENT_TIMEOUT_SECONDS}s"
    fi
    fail "client command failed for $run_name with exit code $client_status"
  fi

  normalize_result_ownership "$WORKSPACE_ROOT/results"
  log "client completed: run=$run_name report=$report_on_host client_log=$client_log_path timing_log=$timing_log_path"

  cleanup_tc_interface
  cleanup_demo_server
  log "run complete: name=$run_name"
done

CURRENT_RUN_NAME="analyze-suite"
analysis_log_path="$(run_log_path harness-analysis)"

analysis_status=0
run_logged_with_timeout \
  "$HARNESS_ANALYZE_TIMEOUT_SECONDS" \
  "$analysis_log_path" \
  env \
  HARNESS_CONFIG="$HARNESS_CONFIG_IN_CONTAINER" \
  docker compose -f "$COMPOSE_FILE" --profile harness run --rm harness analyze-suite --config "$HARNESS_CONFIG_IN_CONTAINER" || analysis_status=$?

if [[ "$analysis_status" -ne 0 ]]; then
  log_file_tail "harness analysis log" "$analysis_log_path"
  if [[ "$analysis_status" -eq 124 || "$analysis_status" -eq 137 ]]; then
    fail "harness analyze-suite timed out after ${HARNESS_ANALYZE_TIMEOUT_SECONDS}s"
  fi
  fail "harness analyze-suite failed with exit code $analysis_status"
fi

normalize_result_ownership "$WORKSPACE_ROOT/results"
CURRENT_RUN_NAME=""
log "suite analysis written under $WORKSPACE_ROOT/results/processed/harness"