# v1.0.0 Release Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Harden zanto for a confident v1.0.0 — lint-clean code, no dead code/stray files, a checklist reconciled with the automated suites, an authored manual-smoke runbook, and green local gates — without expanding scope. CI runs, tagging, and the release build are the user's.

**Architecture:** Three workstreams — Cleanup (hygiene + lint), Working-features (reconcile checklist + author smoke runbook), Tested-app (local gates green + version coherence). Mechanical and additive; **no behavioral refactors**.

**Tech Stack:** Rust (cargo clippy/fmt/test), pnpm (svelte-check, Playwright), git.

## Global Constraints

- **The user owns:** running GitHub CI, tagging the release, building/publishing installers. This plan does NOT tag, push, or run CI.
- **No behavioral refactors.** Clippy/fmt fixes must not change runtime behavior; `cargo test` (149 tests) staying green is the safety net.
- No new dependency.
- Deferred-by-decision items stay deferred (custom endpoints, niche `ChatOptions`, keychain fallback). Brittle-CSS test selectors are left as-is.
- Live-model / OS / vision rows are **authored into the runbook**, not executed by the agent (need the user's keys + desktop session).
- All versions are currently `1.0.0` and must stay coherent across `crates/*/Cargo.toml`, `crates/zanto-desktop/package.json`, `crates/zanto-desktop/src-tauri/tauri.conf.json`.
- Run cargo from repo root; pnpm from `crates/zanto-desktop/`.

---

### Task 1: Repo hygiene

**Files:**
- Modify: `.gitignore` (repo root)
- Delete: `hello.txt` (repo root)
- Add (commit): `docs/specs/*.md` (currently untracked)

- [ ] **Step 1: Confirm `hello.txt` is scratch, then remove it**

Run: `cat hello.txt`
Expected: scratch content ("hello from zanto…"). Then: `git rm -f hello.txt` (or `rm hello.txt` if untracked — check `git ls-files hello.txt`).

- [ ] **Step 2: Gitignore the SDD scratch dir**

Append to the repo-root `.gitignore`:
```
# Subagent-driven-development scratch (ledgers, briefs, review diffs)
.superpowers/
```

- [ ] **Step 3: Stage the loose spec/plan docs + hygiene changes**

The untracked design/plan docs under `docs/specs/` are real project artifacts — commit them. Run:
```bash
git add .gitignore docs/specs/*.md
git rm -f hello.txt 2>/dev/null || true
git status --short
```
Expected: `.gitignore` modified, `docs/specs/*.md` added, `hello.txt` deleted; `.superpowers/` no longer listed as untracked.

- [ ] **Step 4: Commit**

```bash
git commit -m "chore: repo hygiene — gitignore .superpowers, drop hello.txt, commit specs"
```

---

### Task 2: Remove dead code in the mock backend

**Files:**
- Modify: `crates/zanto-desktop/src/lib/mock/backend.ts`

**Interfaces:** `resetBackend()` is exported but has zero callers (Playwright page isolation resets module state on each `page.goto`). Remove it.

- [ ] **Step 1: Confirm zero callers**

Run: `cd crates/zanto-desktop && grep -rn "resetBackend" src tests` 
Expected: only the definition (and maybe its own internal references) — no external call sites. If a caller exists, STOP and report (don't remove a used function).

- [ ] **Step 2: Remove `resetBackend` and leave a boundary note**

Delete the `export function resetBackend() { … }` block. Where it was, leave a one-line comment:
```ts
// Note: mock state (interrupted/errorArmed/pinned/nextPinId) resets naturally —
// each Playwright test loads a fresh page, re-evaluating this module.
```
Remove any now-unused module state ONLY if it's truly unreferenced after this (keep `interrupted`/`errorArmed`/`pinned` — they're used by handlers).

- [ ] **Step 3: Verify suites still green**

Run: `cd crates/zanto-desktop && pnpm check && pnpm test:ui`
Expected: `pnpm check` 0 errors; `pnpm test:ui` all pass (same count as before).

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/mock/backend.ts
git commit -m "chore(desktop): remove dead resetBackend mock helper"
```

---

### Task 3: Lint-clean zanto-core + zanto-cli (clippy + fmt)

**Files:**
- Modify: source files under `crates/zanto-core/` and `crates/zanto-cli/` as clippy/fmt dictate.

- [ ] **Step 1: Format the whole workspace**

Run: `cargo fmt --all`
Then: `cargo fmt --all --check`
Expected: second command exits 0 (no diff). Review `git diff` — formatting only, no logic change.

- [ ] **Step 2: See the clippy warnings for core + cli**

Run: `cargo clippy -p zanto-core -p zanto-cli --all-targets 2>&1 | grep "warning:"`
Expected: a list (~17+ in core). Read each.

- [ ] **Step 3: Fix every warning without changing behavior**

Apply the idiomatic fix clippy suggests for each (needless clones/borrows, redundant closures, `…or_insert_with`, etc.). Do NOT alter runtime behavior. For a genuine false-positive, add a narrow `#[allow(clippy::<lint>)]` with a one-line `// reason:` comment rather than a broad allow. Re-run until clean:
```bash
cargo clippy -p zanto-core -p zanto-cli --all-targets 2>&1 | grep -c "warning:"   # → 0
```

- [ ] **Step 4: Prove no behavioral change**

Run: `cargo test -p zanto-core -p zanto-cli`
Expected: all pass (core 115, plus cli/e2e). If any test changed behavior, revert that fix and use `#[allow]` instead.

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-core crates/zanto-cli
git commit -m "style(core,cli): cargo fmt + clippy clean (no behavior change)"
```

---

### Task 4: Lint-clean zanto-desktop (src-tauri clippy)

**Files:**
- Modify: source files under `crates/zanto-desktop/src-tauri/` as clippy dictates.

**Interfaces:** Consumes Task 3's `cargo fmt --all` (already formatted the desktop crate too).

- [ ] **Step 1: See the desktop clippy warnings**

Run: `cargo clippy -p zanto-desktop --all-targets 2>&1 | grep "warning:"`
Expected: a list. Read each. (This builds the Tauri lib — needs the local GTK/webkit dev libs, which are present.)

- [ ] **Step 2: Fix every warning without changing behavior**

Same discipline as Task 3 Step 3 — idiomatic fixes, narrow `#[allow]` + reason only for false-positives. Re-run until:
```bash
cargo clippy -p zanto-desktop --all-targets 2>&1 | grep -c "warning:"   # → 0
```

- [ ] **Step 3: Prove no behavioral change**

Run: `cargo test -p zanto-desktop`
Expected: all pass (desktop lib 24 + contract 10). 

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src-tauri
git commit -m "style(desktop): clippy clean src-tauri (no behavior change)"
```

---

### Task 5: Reconcile the checklist with automated coverage

**Files:**
- Modify: `docs/zanto-test-checklist.csv`

- [ ] **Step 1: Update statuses programmatically (preserve quoting)**

Use Python's `csv` module (multi-line quoted cells; do NOT hand-edit). Set `Status = "pass"` for rows now green in the local automated suites:
- R-1…R-9 (all automated, CI suites green)
- C-1…C-12 (all automated)
- A-2 (chart) — its fixed-retest is now covered by R-1; set `pass`
- C-12 — fixed-retest now covered by the C-12 spec; set `pass`

Leave F-2 and CO-1 as `fixed-retest` (live-model; they go to the runbook). Leave all other rows unchanged. Script shape:
```python
import csv
rows = list(csv.reader(open("docs/zanto-test-checklist.csv")))
h = rows[0]; si = h.index("Status")
auto_pass = {f"R-{i}" for i in range(1,10)} | {f"C-{i}" for i in range(1,13)} | {"A-2"}
for r in rows[1:]:
    if r[1].strip() in auto_pass:
        r[si] = "pass"
csv.writer(open("docs/zanto-test-checklist.csv","w")).writerows(rows)
```
(C-12 is already in the `C-*` set. Do not touch F-2/CO-1.)

- [ ] **Step 2: Verify integrity + the intended changes**

Run:
```
python3 -c "import csv; r=list(csv.reader(open('docs/zanto-test-checklist.csv'))); assert all(len(x)==len(r[0]) for x in r); h=r[0]; si=h.index('Status'); print('rows',len(r)); print([(x[1],x[si]) for x in r[1:] if x[1].strip() in {'R-1','C-1','C-12','A-2','F-2','CO-1'}])"
```
Expected: row count unchanged (96), no ragged columns; R-1/C-1/C-12/A-2 show `pass`, F-2/CO-1 still `fixed-retest`.

- [ ] **Step 3: Commit**

```bash
git add docs/zanto-test-checklist.csv
git commit -m "docs: reconcile checklist statuses with automated coverage"
```

---

### Task 6: Author the release-smoke runbook

**Files:**
- Create: `docs/release-smoke.md`

- [ ] **Step 1: Write the runbook**

Create `docs/release-smoke.md` — a concise, ordered manual checklist for the user to run before tagging. Group by what mocks can't cover. For EACH item give: **Precondition · Steps · Expected**. Cover exactly these (cross-referencing the checklist IDs):

- **Setup:** S2 — configure a provider + API key (keychain or env), confirm `has_key`.
- **Core live model:** C-1/C-10 against a real provider — a real streamed turn; an induced error (bad endpoint) shows the error card + Retry recovers.
- **Fixed-retests (live model):** F-2 — add a transaction via chat + "this month summary" persists & totals correctly; CO-1 — select a skill, confirm it steers the reply AND persists across an app restart.
- **OS / window:** W-1 window state persistence; W-2 single instance; W-3 turn-done notification (unfocused); W-4 approval notification (unfocused).
- **Vision / web / docs:** DOC-4 (or FLOW-3) one vision turn on a multimodal provider; CO-3 web fetch of a public URL; DOC-2 attach a doc and "summarize it".
- **HITL:** H-1 permission approval overlay (allow once/session/forever/deny); H-2 agent ask-form round-trip.
- **Finance-advanced smoke (best-effort):** FS-1 import `docs/sample-statement.csv` (map columns, account=Checking); FS-2 create accounts; FS-3 set a budget + goals; then confirm dashboard KPIs, Budgets bars, Subscriptions, Trends, Accounts/net-worth, Goals, and Forecast surfaces render without error. Note: exhaustive value-checking is NOT required (logic is Rust-tested) — this is a render/no-crash smoke.
- **FLOW smoke:** FLOW-1…8 happy-path walkthroughs; mark pass or note defects.

End the doc with a **triage rule**: a break in a *core* path blocks the release; a break in an *advanced-finance* path becomes a `known_issues.md` entry, not a blocker.

- [ ] **Step 2: Verify coverage**

Re-read the doc against the checklist; confirm every live-model/OS/vision in-scope row and the Finance/FLOW smoke are represented, each with Precondition/Steps/Expected. No "TODO"/placeholder lines.

- [ ] **Step 3: Commit**

```bash
git add docs/release-smoke.md
git commit -m "docs: add v1 release manual-smoke runbook"
```

---

### Task 7: Final local gate + version/doc coherence (no tagging)

**Files:**
- Modify (only if a mismatch is found): version strings / `CHANGELOG.md` / `README.md`.

- [ ] **Step 1: Run all local gates green**

```bash
cargo fmt --all --check          # 0 diff
cargo clippy --workspace --all-targets 2>&1 | grep -c "warning:"   # 0
cargo test                        # 149 passed
cd crates/zanto-desktop && pnpm check && pnpm test:ui   # 0 errors; all specs pass
```
Expected: every gate clean/green. If anything regressed from Tasks 3–4, fix before proceeding.

- [ ] **Step 2: Version coherence check**

Run:
```bash
cd /home/lazy/dev/github/local-work
grep -m1 '^version' crates/zanto-core/Cargo.toml crates/zanto-cli/Cargo.toml crates/zanto-desktop/src-tauri/Cargo.toml
grep -m1 '"version"' crates/zanto-desktop/package.json crates/zanto-desktop/src-tauri/tauri.conf.json
```
Expected: all `1.0.0`. If any disagree, align them to `1.0.0` and note it. (Do NOT tag or push.)

- [ ] **Step 3: CHANGELOG/README accuracy**

Confirm `CHANGELOG.md` [1.0.0] and `README.md` describe the shipped scope. Add a one-line note (in the CHANGELOG Highlights or a "Notes" line, and/or README) that the **Finance v0.2–v0.5 features are newer and less-exercised**. Keep edits minimal and factual.

- [ ] **Step 4: Commit (if Step 2/3 changed anything)**

```bash
git add -A
git commit -m "docs: note Finance-advanced maturity + confirm v1.0.0 version coherence"
```
(If nothing changed in Steps 2–3, skip the commit.)

---

## Self-Review

**Spec coverage:**
- Workstream A: repo hygiene (T1), dead code (T2), lint core/cli (T3), lint desktop (T4), docs accuracy (T7.3). ✓
- Workstream B: reconcile checklist (T5); F-2/CO-1 + Finance/FLOW smoke authored into the runbook (T6). ✓
- Workstream C: local gates green (T7.1), runbook authored (T6), version coherence (T7.2). ✓
- Out of scope (CI/tag/build) correctly excluded. ✓

**Placeholder scan:** Clippy tasks (T3/T4) specify exact commands + the fix discipline + the green-gate rather than enumerating unknown warnings — that is tool-driven mechanical work, not a placeholder. The runbook (T6) lists exact rows/format. No TBD/TODO.

**Type/consistency:** `resetBackend` removal (T2) matches the mock state it must NOT remove (interrupted/errorArmed/pinned). CSV status set is consistent (R-1…9, C-1…12, A-2 → pass; F-2/CO-1 untouched). Version targets all `1.0.0`.

**Risk note:** T3/T4 clippy fixes are the only behavioral risk; each task gates on `cargo test` green and falls back to a narrow `#[allow]` for false-positives, so behavior is protected. T4 requires the Tauri build (GTK/webkit dev libs present locally).
