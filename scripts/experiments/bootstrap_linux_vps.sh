#!/usr/bin/env bash
set -euo pipefail

log() {
  printf '[bootstrap_linux_vps] %s\n' "$*"
}

fail() {
  log "ERROR: $*" >&2
  exit 1
}

require_sudo() {
  sudo -n true >/dev/null 2>&1 || fail "passwordless sudo is required for VPS bootstrap"
}

install_apt_packages() {
  local packages=(
    ca-certificates
    curl
    git
    iproute2
    jq
    util-linux
  )

  log "installing or updating core host packages"
  sudo apt-get update
  sudo apt-get install -y --no-install-recommends "${packages[@]}"
}

install_buildx_if_missing() {
  if docker buildx version >/dev/null 2>&1; then
    log "docker buildx already available"
    return 0
  fi

  log "installing docker buildx plugin"
  if sudo apt-get install -y --no-install-recommends docker-buildx-plugin; then
    :
  elif sudo apt-get install -y --no-install-recommends docker-buildx; then
    :
  else
    fail "failed to install docker buildx support"
  fi

  docker buildx version >/dev/null 2>&1 || fail "docker buildx is still unavailable after installation"
}

ensure_rust_toolchain() {
  if [[ -x "$HOME/.cargo/bin/cargo" && -x "$HOME/.cargo/bin/rustc" ]]; then
    log "rust toolchain already installed"
  else
    log "installing rustup toolchain for $(id -un)"
    curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --component clippy rustfmt
  fi

  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
  rustup show active-toolchain >/dev/null 2>&1 || rustup default stable
  cargo --version
  rustc --version
}

verify_required_commands() {
  local commands=(git jq docker tc ip nsenter curl)
  for command_name in "${commands[@]}"; do
    command -v "$command_name" >/dev/null 2>&1 || fail "missing required command after bootstrap: $command_name"
  done

  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"
  command -v cargo >/dev/null 2>&1 || fail "cargo missing after bootstrap"
  command -v rustc >/dev/null 2>&1 || fail "rustc missing after bootstrap"
}

verify_repo_inputs() {
  [[ -f Cargo.toml ]] || fail "run this script from the quinn-amc repository root"
  [[ -f scripts/experiments/run_linux_vps_suite.sh ]] || fail "missing VPS runner script"
  [[ -f data/processed/manifests/sintel_trailer_replay.json ]] || fail "missing processed replay manifest under data/processed/manifests"
}

main() {
  require_sudo
  install_apt_packages
  install_buildx_if_missing
  ensure_rust_toolchain
  verify_required_commands
  verify_repo_inputs

  log "bootstrap complete"
}

main "$@"