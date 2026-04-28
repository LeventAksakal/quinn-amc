---
name: "Run Sprint Master"
description: "Use when you want the master sprint agent to orchestrate a sprint, split work across worker agents, launch post-wave QA, integrate the results, and report progress. Keywords: run sprint master, orchestrate sprint, delegate workstreams, master worker flow, sprint QA."
agent: "Sprint Master"
model: "GPT-5 (copilot)"
argument-hint: "Sprint goal or workstreams to orchestrate"
---

Use the Sprint Master agent to orchestrate the requested sprint work for this workspace.

Requirements:

- Start from [Sprint 1 status and vision](../../sprints/s1/00-status-and-vision.md) and the related workstream plans under [sprints/s1](../../sprints/s1/).
- Break the work into explicit worker packages whenever safe parallelism exists.
- Use `Sprint Worker` subagents for delegated implementation tasks.
- After each completed worker wave, launch `Sprint QA` subagents for scoped validation of the finished slices.
- Reconcile worker outputs into one integrated result.
- Reconcile QA outputs into the final sprint status.
- Update sprint artifacts if the delegated work changes the sprint status or execution model.
- Report which tasks were delegated, which were integrated, and which remain blocked.
