# Remediation Plan — review findings + tested CSV failures

**Date:** 2026-06-18 · **Sources:** [code review](../reviews/2026-06-18-finance-code-review.md) + `docs/zanto-test-checklist.csv` partial/fail rows from the user's `pnpm dev` pass.
**Status:** Implemented 2026-06-19 (Batches 0–5). See the completion table below.

## Completion status — 2026-06-19

| Batch | Items | Status |
|---|---|---|
| 0 | B0-1, B0-2, B0-3 | ✅ done (earlier) |
| 1 | B1-1, B1-2 | ✅ done (earlier) |
| 2 | B2-1, B2-2, B2-4 | ✅ done (earlier) · **B2-3 ✅ done** (one-time legacy backfill, `migrate_legacy_transactions`) |
| 3 | B3-1, B3-2, B3-3, B3-4 | ✅ **done** (XLSX serial dates, accounting-negative money parse, truncation/malformed reporting, `insert_batch` atomic import) |
| 4 | B4-1, B4-2, B4-4, B4-5 | ✅ **done** (stale-overview refetch, stable row keys, coalesced nudge, source validation + in-app `ConfirmDialog`). B4-3 folded into B1-2. |
| 5 | B5-1, B5-2, B5-3 | ✅ **done** (`renders_as_block` segment flag; block-reference persistence; notify rate/length caps + chart bounds) |
| 5 | B5-4 | ◑ **partial** — generic `latest_singleton`/`save_singleton` with update-in-place (fixes H6 growth + dedups 5× store boilerplate) ✅; `import.rs` extracted ✅. **Remaining (organizational, zero behavior change):** extract `aggregate.rs`/`stores.rs`; break up `compute_overview`. |
| 6 | Auto context mgmt | ▫ spec only — deferred feature (build later) |

Verify gate at completion: `cargo test` (core 96 + desktop 23 pass), `cargo build`, `pnpm check` (0 errors) + `pnpm build:web` all green. Manual `pnpm dev` smoke-test of the changed flows still pending (user gate).

This merges two streams: (a) the brutal review's P0/Critical/High findings, and (b) the **confirmed runtime failures** the user found by actually running the app. The tested failures go first — they're verified, user-facing, and currently **block further manual testing** (the chart crash poisons every screen that renders a chart).

---

## Batch 0 — Confirmed runtime failures (fix first; they block the rest of the test pass)

| ID | Source | Sev | Problem | Fix |
|----|--------|-----|---------|-----|
| **B0-1** | CSV A-2 + review F-Chart | **P0** | `Chart.svelte:40` sets `title: d.title ? {…} : undefined`. ApexCharts reads `cnf.title.text` unconditionally → `TypeError: undefined is not an object (cnf.title.text)` → **every titleless chart fails to render** (titled ones render — hence "some charts not rendered"). Console confirms `Chart.svelte:83` + `apexcharts.js:15367`. | Always pass a defined title object: `title: { text: d.title ?? "", style: {…} }`. Also harden the `$effect`/`onMount` race (review F-Chart): guard `updateOptions` and don't double-build; consider `try/catch` around `chart.render()`. Re-test R-1, A-2, SS-3, FB-5 (trends chart) after. |
| **B0-2** | CSV C-12 | **High** | Link panel is a **white page** — `<iframe src={promotedLink}>` loads blank in the WebKitGTK webview (X-Frame-Options/CSP/iframe limits). The 4s `embedFailed` fallback doesn't trigger because the iframe fires `load` on a blank doc. User: "iframe will not work, should be a webview/webkit window." | Replace the in-panel iframe with a Tauri **child webview** (`WebviewWindow`/`Webview` via `@tauri-apps/api/webview`) positioned in the panel, OR — simpler and robust — drop the embed entirely and make the panel a clean "open externally" card (host + favicon + Open in browser + Copy), since reliable in-app embedding of arbitrary sites isn't achievable with an iframe. Decide embed-vs-card before building (needs a quick decision). |
| **B0-3** | CSV C-12 console | **Med** | Console shows `window.__TAURI_INTERNALS__ is undefined` (HitlForm.svelte:44, Composer.svelte) + `Data URL decoding failed` for base64 images. Suggests either a web-only `pnpm dev` (no Tauri runtime) or the model still emitting base64-PNG "charts". | Confirm whether the user ran `pnpm dev` (web) vs `pnpm tauri dev` (the Tauri shell); Tauri APIs only exist in the latter. The base64 "charts" are the weak model bypassing the `chart` tool — covered by B0-1 (once charts render the model has a working path) + reinforce the `chart` tool in the prompt. Document the correct launch command in the CSV S-1 row. |

---

## Batch 1 — Review P0s (shipped, verified against code)

| ID | Source | Sev | Problem | Fix |
|----|--------|-----|---------|-----|
| **B1-1** | review P0-1 | **P0** | `do_save_widgets` (`finance/mod.rs:529`) only accepts `kpi\|chart\|table`; `default_widgets()` ships `budget/subscriptions/accounts/goals/forecast`. **Editing + saving the dashboard silently deletes those 5 widget kinds.** | Widen the whitelist to all real kinds, sourced from one shared `const` so the validator can't drift from the builder/defaults again. ~1 line + a test asserting a default-widget round-trips through save. |
| **B1-2** | review P0-2 | **P0 (arch)** | Confused deputy: parse is permission-checked (`ipc/finance.rs`), but `run_app_action`→`import_transactions` is ungated and trusts client-supplied `rows`. | Make import server-side from a permission-checked path: `finance_parse_statement` returns a token; `import_transactions` takes `{token, mapping, account}` and the IPC layer re-reads/parses the checked file. Kills the 5000-row JSON round-trip (review F-C2) at the same time. |

---

## Batch 2 — Critical financial correctness (silent wrong numbers)

| ID | Source | Sev | Problem | Fix |
|----|--------|-----|---------|-----|
| **B2-1** | review C1 | **Critical** | Renaming/deleting an account silently drops its historical transactions/transfers from net worth (`compute_account_balances` no-ops on missing account); `balance` and `net_worth` then disagree. | Accumulate balances for **all** account names seen in transactions; surface an "Unlinked" bucket in net worth, or forbid rename/delete of an account with transactions (offer merge). |
| **B2-2** | review H7 | **High** | A goal linked to a renamed account reads balance 0 → debt goal falsely shows `complete=true`. | Distinguish "account not found" from "balance 0"; mark the goal unlinked/broken. Same fix family as B2-1. |
| **B2-3** | review C2 | **High** | No migrations; missing-`type` legacy rows count as **expenses**, missing-`account` pin to "Cash". The test celebrates the expense-default. | Add a one-time data migration (or a versioned normalize) that stamps explicit `type`/`account` on legacy rows; stop treating lossy defaulting as correct. At minimum document the limitation and don't assert it as intended. |
| **B2-4** | review C3 | **High** | `import_hash` omits `account` → distinct rows (same date/amount/merchant across two accounts) collapse to one `duplicate_skipped`. | Include `account` in the hash input. |

---

## Batch 3 — Import robustness (the bulk-entry path is where silent loss hurts most)

| ID | Source | Sev | Problem | Fix |
|----|--------|-----|---------|-----|
| **B3-1** | review H1 | **High** | XLSX date cells likely import as Excel **serial numbers** (`"45810"`) → all date-derived features break for XLSX. | Use calamine's typed `Data::DateTime`; convert serials to `YYYY-MM-DD`. Until fixed, restrict import to CSV and say so. |
| **B3-2** | review H2 | **High** | `coerce_amount` drops trailing-minus debits (`"5-"` → 0 → row dropped) and mis-parses EU formats; silent zeros vanish. | Real money parsing: handle trailing-minus, locale separators; **never silently drop** a zero-coerced row — report it in the import `errors`. |
| **B3-3** | review H3 | **Med** | `parse_table` truncates at 5000 rows and skips malformed CSV rows **silently**. | Return `truncated`/`total_seen` + a malformed-row count; surface in the import result and the Import UI. |
| **B3-4** | review M3 | **Med** | `do_import_transactions` is 1000+ lock cycles, no surrounding transaction; partial failure leaves a half-imported statement. | Wrap the batch in one DB transaction; roll back on fatal error (keep per-row dedupe-skip as success). |

---

## Batch 4 — Frontend correctness

| ID | Source | Sev | Problem | Fix |
|----|--------|-----|---------|-----|
| **B4-1** | review F-C1 | **High** | `load()` nulls `overview` on every refetch → editors tear down to a skeleton mid-edit; notify `$effect`s re-fire on every refresh. | Keep stale `overview`; swap on success; separate `loading` flag. Fixes several bugs at once. |
| **B4-2** | review F-H3 | **High** | `AccountsEditor`/`GoalsEditor` use `{#each rows as r,i (i)}` (index keys) with remove → bound inputs re-associate to the wrong row. | Stable per-row client id; key by it. |
| **B4-3** | review F-H1/F-C2 | **High** | Import column mapping sends header **names** (breaks on duplicate/empty headers) and round-trips 5000 rows through JSON twice. | Map by column **index**; fold into B1-2's server-side-token import. |
| **B4-4** | review F-H2 | **Med** | Over-budget + pace both fire native notifications for the same category; notify-before-persist re-notifies on failure. | Coalesce into one summary notification; persist dedup key **before** notify. |
| **B4-5** | review F-M / L3 | **Med** | `kpiValue` shows confident `$0.00` for unknown sources; finance IPC is all `any`; MoM block + accounts list duplicated; `window.confirm` vs toasts. | Validate `source` against a known set; type the finance IPC payloads; extract shared `AccountList`/`MoMChip`/progress-bar components; in-app confirm. |

---

## Batch 5 — Architecture / hygiene (do before v0.6)

| ID | Source | Sev | Problem | Fix |
|----|--------|-----|---------|-----|
| **B5-1** | review A1 | **High** | Tool-call hiding at render time desyncs `stepCount` ("0 steps" for a 2-tool turn), breaks WorkflowGroup grouping, and is keyed on un-scoped bare tool names. | Flag the segment "renders as block" when the result is `AppResult::Block`; filter on that, not a name set. |
| **B5-2** | review A2 | **Med** | Hidden tool `output` (e.g. 500-row import result) is persisted into message metadata and re-parsed every reload. | Persist a block reference, not raw output, for block-rendered tools. |
| **B5-3** | review A3/A4 | **Med** | `notify()` is an unauthenticated native-notif primitive; `chart`/`import` have no input bounds. | Rate-limit + length-cap `notify`; cap chart series length + import row count. |
| **B5-4** | review A5 | **Med** | `finance/mod.rs` is a 1700-line god-file; insert-only store boilerplate copy-pasted 5×; `compute_overview` is a 100-line accreting function; unbounded singleton growth (review H6). | Split `aggregate.rs`/`import.rs`/`stores.rs`; a single generic `latest_singleton`/`save_singleton` helper that **updates** instead of insert-only (uses the new `DataStore::update`); break up `compute_overview`. |

---

## Deferred / not-a-bug

- **CSV C-7** (paste expander sub-view in user bubble) — user explicitly marked "v2/p2, current one works fine." Out of scope.
- **Blank-status CSV rows** (R-1…R-9, FV/FB/FI/FA/FG, DOC-4, F-1/F-3, CO-2, FLOW-*) — **untested, not failures.** They're blocked behind B0-1 (charts) and B0-2/B0-3 (panel/launch). Re-run them after Batch 0.
- **Review H4 (f64 money), H5 (pace future-dating), M-series** — real but lower blast radius; schedule after Batches 0–2.

---

## Recommended execution order

1. **Batch 0** (B0-1 chart, B0-2 panel, B0-3 launch doc) — unblocks the whole manual test pass. B0-1 is ~10 lines and the highest value action in this entire plan.
2. **B1-1** (widget whitelist — one line, stops silent data loss).
3. **B2-1/B2-2** (orphaned-account money loss + goal false-complete — the worst correctness bugs).
4. **B1-2 + B3-x + B4-3** (import: server-side token + permission + robustness, done together).
5. **B4-1/B4-2** (frontend refetch + index keys).
6. **B5-x** (architecture) before any v0.6 feature.

Each batch ends by **re-running the relevant CSV rows in `pnpm dev`** — the missing discipline that let the P0s ship. Add one integration test per IPC command and a smoke test of the import + dashboard-save flows as part of Batch 1/3.

---

## Batch 6 — Automatic context management (feature; supersedes the manual turn count)

**Status:** parked / planned (CSV CO-2). Replaces the stopgap manual "Summarize beyond N turns" setting (commit `bc3c520`) with automatic, model-aware context management + a visible indicator. Spec-level; build later.

**Why:** a fixed turn count is the wrong model — a user shouldn't guess a number, and the current behavior is **invisible** (summarization is folded into the prompt as a hidden system message, so it's untestable by eye even when it works). Context limits are a function of the **configured model's window** and **actual token usage**, not turns.

**Goal:** the assistant automatically keeps the conversation within the configured model's context window, summarizing older turns just before overflow, and shows the user when it has done so. No manual tuning required.

**Approach (deterministic where possible):**
1. **Per-model context window.** A small lookup (provider+model → window tokens) with sensible fallbacks (Anthropic ~200k, OpenAI 128k, Ollama model-dependent → conservative default like 8k unless overridden). Expose an optional Settings override for the window when a model isn't in the table.
2. **Token accounting.** genai returns usage (prompt/completion tokens) per response — accumulate it on the session. For the not-yet-sent tail, estimate with a cheap heuristic (~4 chars/token) so the trigger doesn't require a tokenizer.
3. **Auto-trigger.** When projected prompt tokens for the next turn exceed a threshold (e.g. ~70–75% of the window), run the existing `summarize_messages` over the oldest turns, collapse them into the running summary (already implemented in `summarize.rs` / `ContextPolicy::Summarize`), and keep the most recent turns verbatim. Effectively `ContextPolicy::Auto { window, headroom }` computed at send time (the policy is already read per-turn after `bc3c520`).
4. **Visible indicator.** A small, dismissible affordance in the chat stream — e.g. a divider "Earlier conversation summarized to fit context" at the point older turns were compacted, and/or a context-usage hint (n% of window). This is what makes the feature verifiable and trustworthy.
5. **Manual override.** Keep the `max_context_turns` setting only as an advanced/manual override (0 = automatic). Default is automatic.

**Tasks (later):**
- T1 core: `ContextPolicy::Auto { window_tokens, headroom_frac }` + a `model_context_window(provider, model)` lookup + token accounting on `Session` (persist cumulative usage).
- T2 core: at send time, choose verbatim-tail vs summarize based on projected tokens; reuse `summarize_messages`.
- T3 desktop: derive the window from the active provider/model; Settings override field; default policy = Auto.
- T4 frontend: the "summarized to fit context" divider + optional context-usage hint.
- T5: tests — token-projection trigger fires at the threshold; verbatim tail preserved; summary persisted; legacy `max_context_turns` still honored as override.

**Acceptance:** with a small-window model, a long conversation auto-summarizes before overflow with no user setting, the chat shows a "summarized" marker, and the model still answers questions that depend on early-but-summarized turns. The manual turns field is optional (0 = auto).
