---
name: "Plan Next Sprint"
description: "Use when you want to plan the next sprint cycle, inspect current sprint state, discuss scope and tradeoffs, ask clarifying questions, and only then create or update the next sprint folder. Keywords: plan next sprint, sprint intake, sprint planning, next cycle planning, sprint 2 planning."
agent: "Sprint Master"
model: "GPT-5 (copilot)"
argument-hint: "Planning goal, desired outcomes, constraints, and open questions for the next sprint"
---

Use the Sprint Master agent to plan the next sprint cycle for this workspace.

Requirements:

- This prompt is for planning the next sprint, not for executing the active Sprint 1 workstream wave.
- Start by gathering repository context from `README.md`, relevant `docs/`, current harness configs under `configs/harness/`, and the current sprint materials under `sprints/s1/`.
- Inspect the current sprint state before proposing new work. At minimum, read `sprints/s1/00-status-and-vision.md` and the current Sprint 1 planning and automation documents that affect sprint boundaries.
- Ask the user clarifying questions about goals, success criteria, timing, risks, and what should or should not roll forward from Sprint 1 before creating or revising Sprint 2 artifacts.
- Discuss tradeoffs, sequencing, and dependencies explicitly. Surface which items are good candidates for parallel worker packages and which items should stay sequenced.
- Only after that planning conversation, create or update the next sprint folder under `sprints/`. For the current repository state, the expected planning target is `sprints/s2/`.
- Keep the new sprint artifacts actionable and aligned with the existing master, worker, and QA workflow style.
- Treat the initial `sprints/s2/` files as planning artifacts until the user confirms the sprint scope.
- Report the resulting proposed sprint structure, open questions, and any blocked decisions.