---
name: gcloud-vm-check
description: "Inspect and validate the quinn-amc GCP VM with gcloud. Use when testing Compute Engine access, SSH connectivity, Docker and tc prerequisites, sudo behavior, harness execution, result ownership, and artifact presence on quinn-amc-vps."
argument-hint: "target VM, zone, and whether to run read-only checks or execute a suite"
user-invocable: true
---

# Gcloud VM Check

Use this skill to validate the single-host GCP experiment environment for quinn-amc.

## When to Use

- Verify `gcloud` auth, active project, and Compute Engine visibility.
- Check SSH access to `quinn-amc-vps`.
- Confirm the VM has the required tooling for the validated Linux runner.
- Test whether the harness and VPS suite runner work on the VM.
- Inspect result ownership, generated artifacts, and post-run permissions.

## Procedure

1. Confirm control-plane access from the local workspace.
2. Inspect the VM definition and verify SSH execution.
3. Validate the remote repo checkout, branch, and Docker toolchain.
4. Check required Linux tools: `tc`, `ip`, `nsenter`, `jq`, `docker`, `cargo`, `rustc`.
5. Verify sudo behavior with `sudo -n true` and a harmless `tc qdisc show` read.
6. Keep in mind that `analyze-suite` only works after raw reports already exist; it is not a replacement for `run-suite`.
7. Run a safe harness validation path first:
   - `cargo check -p harness`
   - if raw reports already exist: `cargo run -p harness -- analyze-suite --config configs/harness/demo_vod_live.json`
   - otherwise: `cargo run -p harness -- run-suite --config configs/harness/demo_vod_live.json`
8. If the goal is full VPS validation, run a suite through `scripts/experiments/run_linux_vps_suite.sh`.
9. Inspect `results/raw/harness/` and `results/processed/harness/` for expected outputs.
10. Inspect ownership and permissions under `results/` after the run.
11. Report findings as: access, prerequisites, execution result, artifact result, permission result, and next actions.

## Remote Sync Note

- Organization policy currently blocks service account key creation with `iam.disableServiceAccountKeyCreation`.
- Do not assume GitHub Actions can authenticate to GCP with a service account key.
- Prefer manual operator sync via `gcloud compute ssh` and `git pull` on the VM.
- If the VM cannot pull from GitHub directly, use `gcloud compute scp` to copy a prepared repository archive and extract it on the host.

## Command Set

Local control-plane checks:

```bash
gcloud auth list
gcloud config list
gcloud compute instances list
gcloud compute instances describe quinn-amc-vps --zone europe-west6-c
```

Safe remote host checks:

```bash
gcloud compute ssh quinn-amc-vps --zone europe-west6-c --command "hostname; whoami; pwd"
gcloud compute ssh quinn-amc-vps --zone europe-west6-c --command "cd /home/leven/quinn-amc && git branch --show-current && docker --version && docker compose version"
```

Remote prerequisite checks:

```bash
gcloud compute ssh quinn-amc-vps --zone europe-west6-c --command "command -v tc ip nsenter jq docker cargo rustc"
gcloud compute ssh quinn-amc-vps --zone europe-west6-c --command "sudo -n true && sudo tc qdisc show dev lo"
```

Harness validation:

```bash
gcloud compute ssh quinn-amc-vps --zone europe-west6-c --command "cd /home/leven/quinn-amc && cargo check -p harness"
gcloud compute ssh quinn-amc-vps --zone europe-west6-c --command "cd /home/leven/quinn-amc && cargo run -p harness -- run-suite --config configs/harness/demo_vod_live.json"
gcloud compute ssh quinn-amc-vps --zone europe-west6-c --command "cd /home/leven/quinn-amc && cargo run -p harness -- analyze-suite --config configs/harness/demo_vod_live.json"
```

Full VPS suite validation:

```bash
gcloud compute ssh quinn-amc-vps --zone europe-west6-c --command "cd /home/leven/quinn-amc && sudo bash scripts/experiments/run_linux_vps_suite.sh configs/harness/vps_baseline_vod_live.json"
```

Artifact and permission inspection:

```bash
gcloud compute ssh quinn-amc-vps --zone europe-west6-c --command "cd /home/leven/quinn-amc && find results -maxdepth 3 -type f | sort"
gcloud compute ssh quinn-amc-vps --zone europe-west6-c --command "cd /home/leven/quinn-amc && ls -l results results/raw/harness results/processed/harness"
```

## Expected Outcomes

- `gcloud` can list and describe `quinn-amc-vps`.
- SSH works without interactive recovery.
- Required host tools exist.
- `sudo -n true` succeeds for the validated operator path.
- Harness analysis runs successfully on the VM.
- Full VPS suite produces raw and processed harness artifacts.
- Result ownership is attributed to the expected operator user rather than lingering as `root`.

## Reporting Format

- Access: auth, project, instance visibility, SSH status.
- Tooling: required binaries found or missing.
- Harness: check result, analyze-suite result, full runner result if executed.
- Artifacts: raw and processed outputs found or missing.
- Permissions: owner and group under `results/`.
- Risks: any blockers that must be fixed before the next controller matrix run.
