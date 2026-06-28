# Spec: v1.0.0 release readiness

**Date:** 2026-06-22
**Goal:** Get zanto to a confidently-shippable v1.0.0 — clean code, a verified core feature set, and a green test gate — without expanding scope. The release artifacts (CHANGELOG, README, `release.yml`, version bumps) already exist; this is the hardening pass before tagging.

## Scope decisions (locked)

- **Feature scope:** the **core** app ships fully verified (chat, sessions, artifacts, basic finance, HITL, workspace, settings/providers). **Finance-advanced** (v0.2–v0.5: FV/FB/FI/FA/FG) ships **best-effort** — its logic is Rust-tested; it gets a single UI smoke pass, not row-by-row verification, and is noted as newer/less-exercised. **FLOW-1…8** workflows get manual smoke only.
- **Release gate:** **local automated suites green** (`cargo test` + Playwright `test:ui` + `pnpm check`) **+ an authored manual smoke runbook** of the core flows mocks can't cover (live model, window/OS, vision, web, document read). **Running GitHub CI and cutting the actual release/tag are the user's — out of this plan's scope.**
- **Cleanup depth:** lint-clean + dead-code + stray-file removal + repo hygiene. **No behavioral refactors.**
- **Deferred (stay deferred):** custom OpenAI-compatible endpoint providers, remaining niche `ChatOptions` (`response_format`/`service_tier`/`cache_control`), app-level secret-store fallback. Brittle-CSS test selectors are left as noted (not churned).

## Workstream A — Cleanup

1. **Lint/format both crates:** `cargo clippy --workspace --all-targets` and `cargo fmt --all --check` → fix all warnings/format diffs (no behavioral change). `pnpm check` (svelte-check) → 0 errors/0 warnings (already clean; keep it so).
2. **Dead code:** remove `resetBackend()` in `crates/zanto-desktop/src/lib/mock/backend.ts` (zero callers; per page-reload isolation it never runs) — or wire it into a Playwright init hook; prefer removal + a one-line comment that page reload is the reset boundary.
3. **Stray files:** remove `hello.txt` at the repo root (confirm it's a scratch file first). Audit for other scratch/debug artifacts.
4. **Repo hygiene:** the loose `docs/specs/*` design+plan docs (split-bridge, regression, chat-ui, this one) are untracked — commit them under `docs/specs/`. Add `.superpowers/` to `.gitignore` (SDD scratch: ledgers, briefs, review diffs — not source).
5. **Docs accuracy:** confirm `CHANGELOG.md` / `README.md` describe the shipped scope; add a one-line note that the Finance v0.2–v0.5 features are newer/less-exercised. `known_issues.md` is already empty (no open bugs) — leave.

**A is done when:** `cargo clippy` + `cargo fmt --check` + `pnpm check` are clean, no dead `resetBackend`/`hello.txt`, `.superpowers/` ignored, loose docs committed.

## Workstream B — Working features (verify the in-scope set)

1. **Reconcile the checklist with current automated coverage.** In `docs/zanto-test-checklist.csv`, set `Status = pass` for the rows now green in CI: R-1…R-9, C-1…C-12. Close the fixed-retest items already covered by automation — **A-2** (chart, via R-1) and **C-12** (link, via the C-12 spec) → `pass`. (Programmatic CSV edit; preserve quoting/integrity.)
2. **Manual retest the 2 remaining live-model fixed-retests:** **F-2** (finance add-transaction + "this month summary" via chat) and **CO-1** (skill selection steers the reply, persists across restart). Record pass/fail in the checklist with the model used.
3. **Finance-advanced UI smoke (best-effort):** run FS-1 (import `docs/sample-statement.csv`) + FS-2/FS-3 (accounts, budget+goals), then confirm the dashboard, Budgets, Subscriptions, Trends, Accounts, Goals, and Forecast surfaces render without error. Annotate the FV/FB/FI/FA/FG rows as `smoke` (or `pass` where clearly working); do not exhaustively verify each computed value (logic is Rust-tested).
4. **FLOW smoke:** quickly exercise FLOW-1…8 happy paths; mark pass or note defects. Any defect found becomes a release decision (fix vs defer-to-known-issues).

**B is done when:** the checklist reflects reality — in-scope core rows `pass`, fixed-retests resolved, Finance-advanced/FLOW annotated smoke, and any new defects triaged (fixed or recorded in `known_issues.md`).

## Workstream C — Tested app (gate artifacts)

> Running GitHub CI and cutting the release/tag are the **user's** — not in this plan. This workstream produces the artifacts that make the gate runnable.

1. **Local automated suites green.** Confirm `cargo test`, `pnpm test:ui`, and `pnpm check` all pass locally on the final branch (they are the same suites CI runs). This is the evidence handed to the user before they run CI.
2. **Author the manual smoke runbook.** Add `docs/release-smoke.md` — a concise, ordered checklist of the live-model / OS / vision rows the user must verify before release: a real streamed chat turn against a configured provider; provider+key save (keychain/env); window persistence + single-instance + the two notifications (W-1…W-4); one vision turn (DOC-4/FLOW-3); web fetch (CO-3); document read (DOC-2); and the Finance-advanced + FLOW smoke (cross-referenced from Workstream B). Each item: precondition, steps, expected. The user runs it and records results; **the agent authors it, does not execute the live-model parts** (needs the user's keys/desktop session).
3. **Version coherence check (no tagging).** Verify the version strings agree across `crates/*/Cargo.toml`, `crates/zanto-desktop/package.json`, and `tauri.conf.json`, and that CHANGELOG/README match — so the user's tag step is clean. Do NOT tag or push.

**C is done when:** local `cargo test` + `pnpm test:ui` + `pnpm check` are green, `docs/release-smoke.md` exists and covers the in-scope core + Finance/FLOW smoke rows, and versions are coherent. The user owns the CI run and the tag/build.

## Out of scope
- **Running GitHub CI; cutting/tagging the release; building/publishing installers — the user owns these.**
- Executing the live-model / OS / vision manual smoke (the agent authors the runbook; the user runs it).
- Exhaustive row-by-row verification of Finance v0.2–v0.5.
- Signing/notarization, auto-update, app-store distribution (documented limitations).
- Behavioral refactors; the deferred backlog features; the brittle-CSS test-selector cleanups.

## Success criteria
- `cargo clippy`/`cargo fmt --check`/`pnpm check`/`cargo test`/`pnpm test:ui` all clean/green **locally**.
- No dead `resetBackend`, no `hello.txt`, `.superpowers/` gitignored, loose docs committed.
- Checklist: in-scope core rows `pass`; F-2/CO-1 retested (or queued in the runbook for the user); Finance-advanced + FLOW annotated; any defects triaged.
- `docs/release-smoke.md` authored, covering the core live-model/OS rows + Finance/FLOW smoke.
- Versions coherent across Cargo.toml/package.json/tauri.conf.json; CHANGELOG/README accurate. (User tags + runs CI + builds.)

## Notes / risks
- Live-model retests (F-2, CO-1) and the OS/window/vision rows need the user's keys + desktop session — the agent **queues them in the runbook** rather than executing them. If the user wants, the agent can drive the non-model UI parts via the run/verify tooling, but model-dependent assertions are the user's pass.
- Finance-advanced shipping best-effort is a deliberate, documented trade-off — if FLOW/finance smoke surfaces a user-facing break in a *core* path, it's a blocker; a break in an *advanced finance* path is a `known_issues.md` entry, not a blocker.
