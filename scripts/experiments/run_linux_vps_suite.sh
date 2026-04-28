#!/usr/bin/env bash
set -euo pipefail

CONFIG_PATH="${1:-configs/harness/vps_demo_vod_live.json}"
WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIG_ABS="$WORKSPACE_ROOT/$CONFIG_PATH"
COMPOSE_FILE="$WORKSPACE_ROOT/compose.yaml"
ACTIVE_TC_INTERFACE=""
CURRENT_RUN_NAME=""

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

[[ -f "$CONFIG_ABS" ]] || fail "config not found: $CONFIG_ABS"
[[ -f "$COMPOSE_FILE" ]] || fail "compose file not found: $COMPOSE_FILE"

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

get_container_host_veth() {
  local container_id="$1"
  local pid
  local iflink
  local host_interface

  pid="$(docker inspect -f '{{.State.Pid}}' "$container_id")"
  [[ -n "$pid" && "$pid" != "0" ]] || fail "container $container_id is not running with a valid PID"

  iflink="$(nsenter -t "$pid" -n cat /sys/class/net/eth0/iflink)"
  [[ -n "$iflink" ]] || fail "failed to resolve eth0 iflink for container $container_id"

  host_interface="$(ip -o link | awk -F': ' -v idx="$iflink" '$1 == idx { print $2; exit }')"
  host_interface="${host_interface%%@*}"
  [[ -n "$host_interface" ]] || fail "failed to resolve host veth for iflink $iflink from container $container_id"

  log "resolved host veth: container=$container_id pid=$pid iflink=$iflink interface=$host_interface"
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

log "building compose services: demo-server demo-client harness"
docker compose -f "$COMPOSE_FILE" build demo-server demo-client harness

for run_name in $RUN_NAMES; do
  CURRENT_RUN_NAME="$run_name"
  scenario_name="$(get_run_field "$run_name" network_scenario)"
  mode="$(get_run_field "$run_name" mode)"
  pace="$(get_run_field "$run_name" pace)"
  vod_deadline_slack_ms="$(get_run_field "$run_name" vod_deadline_slack_ms)"
  report_in_container="/workspace/results/raw/harness/${run_name}_report.json"
  report_on_host="$WORKSPACE_ROOT/results/raw/harness/${run_name}_report.json"
  scenario_json="$(get_scenario_json "$scenario_name")"
  [[ -n "$scenario_json" ]] || fail "run $run_name references unknown scenario $scenario_name"

  log "starting run: name=$run_name scenario=$scenario_name mode=$mode pace=$pace vod_deadline_slack_ms=${vod_deadline_slack_ms:-unset}"

  cleanup_tc_interface
  cleanup_demo_server

  rm -f "$report_on_host"
  rm -f "$SERVER_CERT_HOST_PATH"
  log "run setup: cleared prior report and certificate outputs for $run_name"

  DEMO_SERVER_REPORT_OUT="$report_in_container" \
  DEMO_SERVER_CERT_OUT="/workspace/results/demo-cert.der" \
  DEMO_SERVER_PORT=5001 \
  docker compose -f "$COMPOSE_FILE" up -d demo-server

  server_container_id="$(wait_for_container_running demo-server)"
  log "demo-server running: container=$server_container_id"
  wait_for_file "$SERVER_CERT_HOST_PATH"
  log "server certificate ready: $SERVER_CERT_HOST_PATH"
  server_host_veth="$(get_container_host_veth "$server_container_id")"
  apply_tc_from_scenario "$server_host_veth" "$scenario_json"

  DEMO_CLIENT_SERVER="demo-server:5001" \
  DEMO_CLIENT_SERVER_NAME="localhost" \
  DEMO_CLIENT_CERT="/workspace/results/demo-cert.der" \
  DEMO_CLIENT_REPLAY_MANIFEST="$REPLAY_MANIFEST_IN_CONTAINER" \
  DEMO_CLIENT_PACE="$pace" \
  DEMO_CLIENT_MODE="$mode" \
  DEMO_CLIENT_VOD_DEADLINE_SLACK_MS="$vod_deadline_slack_ms" \
  docker compose -f "$COMPOSE_FILE" run --rm demo-client

  log "client completed: run=$run_name report=$report_on_host"

  cleanup_tc_interface
  cleanup_demo_server
  log "run complete: name=$run_name"
done

CURRENT_RUN_NAME="analyze-suite"

HARNESS_CONFIG="$HARNESS_CONFIG_IN_CONTAINER" \
docker compose -f "$COMPOSE_FILE" run --rm harness analyze-suite --config "$HARNESS_CONFIG_IN_CONTAINER"

CURRENT_RUN_NAME=""
log "suite analysis written under $WORKSPACE_ROOT/results/processed/harness"