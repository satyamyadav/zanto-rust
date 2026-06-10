Implement a spec'd change.

**Model:** sonnet — implementation. Drop to haiku for purely mechanical single-file steps. Escalate to opus if a step needs real design judgment or tricky debugging.

`$ARGUMENTS` = path to a spec file under `docs/specs/`, or a short description of an already-approved change.

---

## What to do

1. **Read the spec** — if given a path, read the full file. If given a description, confirm the change is small and self-contained before proceeding (otherwise run `/spec` first).

2. **Guard** — if the spec has not been reviewed and approved by the user, stop: "Not approved yet — run `/spec` first, or confirm you want to proceed."

3. **Implement** each numbered step in order. After completing each step, tick its checkbox in the spec file.

4. **Verify** — run `cargo build` after each meaningful step; fix any errors before moving on. A green build does not prove behaviour.

5. **Manual test** — run the affected CLI flow and confirm the acceptance criteria end-to-end. If you cannot run a step interactively (e.g. requires a live LLM), describe the exact command and expected output and ask the user to confirm.

6. **Report** — show which files changed and which acceptance criteria were confirmed (and how).

---

## Rules

- Follow the spec exactly. Do not add features, refactor surrounding code, or fix unrelated issues.
- Do not tick a step checkbox until the code change is actually made and `cargo build` is green.
- If the spec turns out to be wrong or incomplete mid-way, stop, describe the problem, and ask whether to update the spec or abort.
- If a step touches a public API (`check()`, `chat()`, `ToolBase` impls, etc.), grep call sites and confirm none is broken.
- `cargo build` green is required but not sufficient — always run the CLI to confirm the changed flow.
