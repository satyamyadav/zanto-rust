Full spec-driven development loop: spec → review → implement.

**Model:**
- Phase 1–2 (spec, revision): **opus** — planning and judgment
- Phase 3 (execute): **sonnet** — invoke `/execute` (carries `model: sonnet`)

No code is written until you approve the spec.

---

## Phase 1 — Spec

Run `/spec $ARGUMENTS`:
- Explore relevant code
- Ask clarifying questions for anything ambiguous (scope, edge cases, API changes, new deps)
- Write spec to `docs/specs/YYYY-MM-DD-<slug>.md`
- Show full spec inline

Then ask: **"Approve this spec, or tell me what to change."**

Wait for response. No code until approved.

## Phase 2 — Revision (if needed)

If changes requested: edit the spec file, show the diff, ask again. Repeat until explicitly approved. Do not touch source files during this phase.

## Phase 3 — Execute

Once approved, run `/execute <spec-path>`:
1. Implement each step in order, ticking spec checkboxes as you go
2. Run `cargo build` after each step; fix before continuing
3. Manual-test the changed flow: run the CLI and confirm each acceptance criterion
4. Show summary: which files changed and which criteria were confirmed

---

## Rules

- Never write implementation code before Phase 3.
- If the user says "cancel" or "abort", stop and leave any partial spec file as-is.
- Keep phases clearly labelled in responses so the user knows which phase is active.
- If a spec step proves wrong mid-implementation, stop and describe the problem — don't silently adapt around it.
