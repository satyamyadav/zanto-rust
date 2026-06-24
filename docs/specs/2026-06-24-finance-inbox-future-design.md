# Finance Inbox — future design (not scheduled)

**Date:** 2026-06-24
**Status:** Future / exploratory. **Not scheduled, not implemented.** This captures the design so it's ready to spec → plan → build when picked up. No positioning here — product mechanics only.

## Concept

A **folder-as-inbox + AI bookkeeper** for the finance app. The user drops financial files (any format) into a watched local directory; the app detects them, parses what it can deterministically, has the AI propose the fuzzy parts (account, column mapping, categories, duplicates, ambiguities), and the user **reviews and confirms** before anything posts. Deliberately replaces bank/cloud sync with *local files + AI-assisted ingestion* — everything stays on the machine.

Design tenets:
- **Structured-first, AI-second.** Parse structured exports deterministically (no AI); use the model only for what it's genuinely good at (categorization, fuzzy column/account mapping, messy PDFs).
- **AI proposes, user confirms.** Nothing mutates the ledger silently. Every AI-driven import is a reviewable plan with a diff and undo.
- **Deterministic math, always.** Totals/balances stay computed in Rust (existing invariant) — the AI orchestrates ingestion, never the accounting.
- **Local-only.** No cloud, no sync, no mobile (out of scope by design). The watched folder + SQLite are the source of truth → backup becomes first-class.

## Reuse — primitives already in the codebase

| Need | Existing piece |
|---|---|
| Multi-format file read | `zanto-core` `read_document` (PDF via pdf-extract; xlsx/xls/ods via calamine; docx) |
| Structured CSV parsing | `csv` crate + finance `do_import_transactions` (column mapping, debit/credit/signed) |
| Dedupe | `import_hash(date, amount, merchant, account)` in `apps/finance/import.rs` |
| Categorization | category rules (`resolve_category`, `save_category_rule`) |
| Accounts / transfers | `save_accounts`, `add_transfer`, net-worth compute |
| Agent orchestration | finance agent tools + the agentic chat loop |
| "Files in a local dir fed to the agent" | `context_sources` + project dir (`config.rs`) |
| Permission boundary | `PermissionGuard` (the watched dir would be an allowed root) |

So most of this is **orchestration + new parsers + a review UX**, not greenfield accounting.

## Architecture — the ingestion pipeline

```
watched dir ──▶ detect/scan ──▶ classify ──▶ parse ──▶ build import plan ──▶ REVIEW (user) ──▶ apply ──▶ posted
                  (new/changed)   (format)    (det. │ AI)   (account, mapping,                 (dedupe,
                                                            dupes, categories,                  audit)
                                                            ambiguities)
```

1. **Detect / scan.** A "scan this folder" action (manual button first; optional watch later). Lists files with a per-file status: `new` · `parsed` · `needs-review` · `imported` · `error`. Track which files/rows already imported (by file hash + row `import_hash`) so re-scanning is idempotent.
2. **Classify.** By extension/magic: structured (CSV/OFX/QFX/QIF/MT940), spreadsheet (xlsx/ods), document (PDF/docx), unknown.
3. **Parse.**
   - **Deterministic (no AI):** OFX/QFX/QIF/MT940 (bank-standard, structured — near "sync" quality), well-formed CSV, simple spreadsheets. These produce structured transactions directly.
   - **AI-assisted:** ambiguous CSV/spreadsheet layouts (which column is amount/date/merchant), and PDF statements (extract text via `read_document`, then the model proposes a transaction table). PDF is the hardest, least-reliable case — treat as best-effort with mandatory review.
4. **Build import plan.** A structured proposal: target **account** (inferred or asked), **column mapping**, **N new vs M duplicates** (via `import_hash`), **category assignments** (rules + AI suggestions), and a list of **flagged ambiguities** needing a decision. The plan is data, not yet applied.
5. **Review (the Inbox UI).** A Finance → **Inbox** tab: the file list + an import-plan review pane — proposed account, the mapping, a preview table (new rows highlighted, duplicates dimmed), editable categories, and ambiguity prompts. The user edits/approves; **confirm** applies, **undo** reverts the batch.
6. **Apply.** Insert non-duplicate rows (dedupe enforced), record an **audit entry** (source file, count, timestamp, the plan used) so an import is traceable/reversible.

## Per-source memory

Key a remembered **source profile** by a stable signature (filename pattern + header/columns fingerprint, or OFX bank/account id). Store: target account, column mapping, and category-rule hints. Then the *second* statement from the same bank imports in one tap — the plan is pre-filled, the user just confirms. This is what turns "AI-guided" into "near-zero-effort over time."

## Trust & safety

- Every AI-proposed import is a **reviewable diff + undo + audit trail**. No silent ledger writes.
- The watched directory is an **explicit allowed root** (PermissionGuard); reads are gated like all FS access.
- Deterministic compute unchanged — AI never produces totals/balances.

## Reliability ↔ privacy tension (the central risk)

The ingestion "magic" scales with model quality: strong cloud model (Claude/Gemini) → reliable mapping/categorization/PDF; small local Ollama → degrades. Mitigations:
- Maximize the **deterministic** path (OFX/QIF/CSV) so most volume needs no AI.
- For AI steps, **suggest, don't auto-apply**, and surface confidence; fall back to manual mapping (with AI as hints) when the model is weak.
- Recommend a capable model for ingestion in the UI when AI parsing is invoked.

## Backup (now load-bearing)

Local files + SQLite are the only copy. Pair the inbox with a simple **export/backup** (e.g. export ledger to CSV/JSON, or a one-click DB backup) so "local-only" isn't "one-disk-failure-from-zero."

## Likely new pieces (sketch — settle when specced)

- **Parsers:** OFX/QFX, QIF (and maybe MT940) → structured transactions (deterministic). Evaluate a crate vs hand-rolled.
- **IPC/core:** `scan_finance_inbox(dir) -> [FileStatus]`; `parse_finance_file(path) -> ParsedBatch | NeedsMapping`; `build_import_plan(parsed, account?, mapping?) -> ImportPlan`; `apply_import_plan(plan) -> {inserted, skipped}`; a source-profile store.
- **AI flow:** an agent tool / scripted flow that, given extracted text or an ambiguous table, returns a proposed mapping + transaction rows for the plan (always routed through review).
- **UI:** the Inbox tab (file list + plan review + confirm/undo), per-source memory editor, backup/export control.

## Suggested phasing (when scheduled)

- **P1 — Deterministic core:** Inbox tab + folder scan + CSV/OFX/QIF parsing + import-plan review/confirm + dedupe + audit. (No AI required → most reliable, biggest friction win.)
- **P2 — Memory + robustness:** per-source profiles (one-tap repeat imports), cross-file overlap dedupe, account inference.
- **P3 — AI-assisted:** ambiguous-layout mapping + PDF statement extraction (AI proposes → review). Confidence + manual fallback.
- **P4 — Durability:** export/backup; optional folder watch (vs manual scan).

## Non-goals (by design)
Direct bank/Plaid sync; cloud sync; mobile; multi-user. The premise is *local files + AI labor*, not connectivity.

## Open questions (resolve at spec time)
- Inbox folder: fixed (`<project>/.zanto/finance-inbox`?) vs user-configured? Manual scan vs live watch for v1?
- Multi-account files (one CSV spanning accounts) — split how?
- Import-plan representation/persistence (so a half-reviewed plan survives a restart)?
- Minimum model capability for the AI-assisted path; how to detect/communicate degradation.
- Backup format + restore flow.
