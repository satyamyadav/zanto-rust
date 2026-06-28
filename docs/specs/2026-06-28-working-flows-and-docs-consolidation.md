# Working flows & docs consolidation

- **Date:** 2026-06-28

## Summary

Standardize the project on the native `/dev`, `/spec`, `/execute` flow (drop
superpowers/context7 usage), add a `docs/working-flows.md` flows charter, update
CLAUDE.md, and reduce `docs/` to a living set (architecture + one product doc +
a `docs/test/` folder) by archiving all completed specs/plans and dropping the
redundant root reference files — after preserving the testing content `trd.md`
uniquely holds into `docs/test/`.

## Motivation

The repo accreted two parallel dev flows: the project's native slash commands
(`.claude/commands/{spec,dev,execute}.md`, writing to `docs/specs/`) and the
**superpowers** plugin flow (brainstorming → writing-plans →
subagent-driven-development, writing to `docs/superpowers/specs|plans/`). The two
conflict and confuse. The owner wants: one simple, native, multi-step **reviewed**
flow; no superpowers, no context7; and a minimal living-docs set. `docs/` is also
bloated (80 files, 1.6 MB) — 51 dated spec/plan files are completed, shipped work.

## Scope

**In scope**
- New `docs/working-flows.md` (the flows charter).
- Rewrite the flow-related parts of `CLAUDE.md`.
- New `docs/test/` folder: preserve `trd.md`'s testing/mock-seam content as
  `docs/test/testing.md`, and move the QA checklist there as a LIVING doc.
- Consolidate `docs/vision/`, `docs/product/`, `docs/design/`, `docs/stories/`
  into one `docs/product.md`.
- Archive all completed dated specs/plans (incl. `docs/superpowers/*`) into
  `docs/archive/`.
- Drop root `trd.md` and `known_issues.md` (after preserving needed content).
- Triage the one-off files (`*.csv`, `release-smoke.md`,
  `agent-branches-archived-*.md`, `backlog.md`).

**Out of scope**
- Uninstalling superpowers/context7 plugins — they live in `~/.claude/plugins`
  (global), not this repo. Non-use is enforced by docs/rules only.
- Any source-code change. This is docs + `.claude` + `CLAUDE.md` only.
- Rewriting the native command files themselves (the owner chose "keep as-is").
- Editing other repos (zanto-site etc.).

## Affected files

**Create**
- `docs/working-flows.md` — the flows charter.
- `docs/test/testing.md` — preserved from `trd.md`'s Testing section.
- `docs/product.md` — consolidated product/vision doc.
- `docs/archive/README.md` — one-line note on what the archive holds.

**Move into `docs/test/` (living docs, NOT archived)**
- `docs/zanto-test-checklist.csv` → `docs/test/qa-checklist.csv` (active QA
  checklist — kept live, per owner).

**Move (git mv → `docs/archive/`)**
- All of `docs/specs/*.md` (51 files) — completed/shipped designs.
- All of `docs/plans/*.md` (4 files).
- All of `docs/superpowers/**` (5 files).
- `docs/reviews/2026-06-18-finance-code-review.md`.
- `docs/agent-branches-archived-2026-06-18.md`.
- `docs/release-smoke.md` (historical launch checklist).

**Consolidate then remove (content folded into `docs/product.md`)**
- `docs/vision/genui-d.md`, `docs/product/finance-next-version-plan.md`,
  `docs/design/micro-app-architecture.md`, `docs/stories/2026-06-13-*.md`.

**Delete (root)**
- `trd.md` — after its Testing section is preserved to `docs/architecture/testing.md`
  and its other sections confirmed covered by `docs/architecture/*`.
- `known_issues.md` — owner chose to drop; its 2 KB of content is superseded by
  git history + active specs. (Flagged under risks.)

**Modify**
- `CLAUDE.md` — Process, Slash commands, Key files sections (details below).
- `docs/zanto-test-checklist.csv`, `docs/sample-statement.csv`,
  `docs/backlog.md` — relocate (see step 7).

**Note (do NOT move/archive)**
- `docs/architecture/*` (7 files) — the living technical reference; refreshed
  only where stale, otherwise untouched.
- `docs/test/` — new living folder (`testing.md` + `qa-checklist.csv`).
- `docs/images/` — referenced assets; keep.
- `CHANGELOG.md` (root) — already exists; keep, unrelated.

## Implementation steps

1. **Preserve trd.md's testing content** (`docs/test/testing.md`)
   - The architecture set has NO testing/mock-seam doc (verified: only
     `permissions.md` mentions "seam", unrelated). `trd.md` §"Testing
     (zanto-desktop)" (single Tauri seam, `--mode mock`, thin-command guardrail,
     golden fixtures + Rust contract test, "Adding a command") is unique.
   - `mkdir docs/test`; create `docs/test/testing.md` (heading `# Testing`) and
     move that section's content verbatim, lightly edited for standalone framing.
     This must happen BEFORE deleting `trd.md`. `docs/test/` is a LIVING doc
     folder (testing reference + QA checklist), not part of the archive.

2. **Confirm remaining trd.md sections are covered, fold any gap**
   (`docs/architecture/*`)
   - trd sections: Overview, Key Dependencies, Source Structure, Configuration,
     Permission System, Tool Architecture, Chat Orchestration, Session/History,
     CLI, Future Roadmap.
   - For each, confirm an architecture/ file covers it (overview, modules,
     permissions, tools, llm/stack-flow, data-model). Where a trd subsection has
     detail the architecture file lacks (e.g. the exact "Tool file contract"
     ordering, the SQLite schema/migration note), fold that paragraph into the
     matching architecture file. Record in the PR which trd subsections were
     folded vs already-covered. Do not delete trd.md until this is done.

3. **Write `docs/working-flows.md`** (the charter)
   - Sections:
     - **Flow:** the only dev flow is the native `/dev` (full loop) / `/spec`
       (spec, no code) / `/execute` (implement a spec). Multi-step, reviewed:
       spec → approve → execute. Specs in `docs/specs/YYYY-MM-DD-<slug>.md`.
     - **Banned tooling:** do NOT use the superpowers plugin or context7 for this
       project. (They are global plugins; just don't invoke them.) No
       brainstorming/writing-plans/subagent-driven/executing-plans skills.
     - **Reviewed flow:** present spec, wait for explicit approval before code;
       run `cargo build` + manual CLI/app verification per step; report what was
       confirmed.
     - **Cross-project rule:** this repo stores NO context, specs, or plans for
       other repos (e.g. zanto-site). If a change is needed there, OUTPUT the
       requirements as a copy-pasteable block for the owner to take to that
       repo's agent — never create files for it here.
     - **Docs rule:** living docs = `docs/architecture/*` + `docs/test/*` +
       `docs/product.md`;
       completed specs/plans go to `docs/archive/`.

4. **Rewrite CLAUDE.md flow sections** (`CLAUDE.md`)
   - **Process** section: keep "never write impl before spec approved"; point to
     `docs/working-flows.md` as the canonical flow; remove any wording that could
     imply superpowers/brainstorming.
   - **Slash commands** table: keep `/spec`, `/execute`, `/dev` rows; add a line
     "Do not use the superpowers or context7 plugins (see `docs/working-flows.md`)."
   - **Key files** table: replace the `trd.md` and `known_issues.md` rows with
     `docs/architecture/` (technical reference), `docs/test/` (testing reference +
     QA checklist), and `docs/product.md`; keep the `docs/specs/` row; add
     `docs/working-flows.md` and `docs/archive/`.
   - Add the cross-project handoff rule (one line, pointing to working-flows.md).

5. **Consolidate the product/vision docs** (`docs/product.md`)
   - Read `docs/vision/genui-d.md`, `docs/product/finance-next-version-plan.md`,
     `docs/design/micro-app-architecture.md`, `docs/stories/2026-06-13-*.md`.
   - Write ONE `docs/product.md`: product vision, the micro-app architecture
     concept (from design/), and a short "directions" list (folding the still-live
     finance-next + the story). Drop superseded/dated detail; this is a living
     summary, not an archive. Then `git rm` the four source files + their now-empty
     dirs (`vision/`, `product/`, `design/`, `stories/`).

6. **Archive completed specs/plans** (`docs/archive/`)
   - `mkdir docs/archive`; `git mv` all `docs/specs/*.md`, `docs/plans/*.md`,
     `docs/superpowers/**`, `docs/reviews/*.md`,
     `docs/agent-branches-archived-2026-06-18.md`, and `docs/release-smoke.md`
     into `docs/archive/` (flat; names already dated/unique — on the rare
     basename collision, prefix the source subdir).
   - Remove the now-empty `docs/specs/`, `docs/plans/`, `docs/superpowers/`,
     `docs/reviews/` dirs.
   - Add `docs/archive/README.md`: "Completed/shipped specs, plans, and reviews.
     Historical reference only — not living docs. New specs go in `docs/specs/`."
   - Re-create an empty `docs/specs/` (with a `.gitkeep`) so the native `/spec`
     flow still has its target dir.

7. **Triage one-off files**
   - `docs/sample-statement.csv` — test fixture for finance import. Move to
     `crates/zanto-desktop/contract/fixtures/` if used by tests, else
     `docs/archive/`. (Check refs first: grep the repo.)
   - `docs/zanto-test-checklist.csv` — manual QA checklist. `git mv` to
     `docs/test/qa-checklist.csv` (kept as a LIVING doc alongside testing.md, per
     owner). Not archived.
   - `docs/backlog.md` — if it lists the 7 upcoming features, fold the live items
     into `docs/product.md` "directions" and archive the file; else archive.

## Edge cases & risks

- **Dropping `known_issues.md`:** removes the only structured known-issues list.
  Mitigation: its content (2 KB) is small and mostly superseded; git history
  retains it; future issues tracked in specs. If the owner later wants it back,
  it's one `git show` away. **Flagged — confirm in review.**
- **Dropping `trd.md`:** safe ONLY after step 1 (testing.md) + step 2 (fold
  gaps). The risk is silently losing the mock-seam/testing doc — step 1 prevents
  it. The PR must list which trd subsections were folded vs already-covered.
- **CLAUDE.md references to moved files:** after the move, grep CLAUDE.md and the
  `.claude/commands/*` for any path that now points into `docs/archive/`
  (e.g. `trd.md`, `known_issues.md`) and fix. The command files reference
  `docs/specs/` (kept) — verify they don't break.
- **`docs/specs/` emptied:** the native `/spec` flow writes there. Re-creating it
  with `.gitkeep` (step 6) keeps the flow working. This very spec lives in
  `docs/specs/` and will itself be archived once this work ships.
- **No new dependency.** Docs-only change; no crate, no `cargo` impact.
- **Reversibility:** everything is `git mv`/`git rm` — fully recoverable from
  history. No content is destroyed, only relocated or summarized.

## Acceptance criteria

User-observable outcomes (verifiable by listing files / reading docs / running
the build, since this is a docs+config change with no runtime behavior):

- [ ] `docs/working-flows.md` exists and states: native `/dev,/spec,/execute`
      only; no superpowers/context7; reviewed multi-step flow; the cross-project
      handoff rule; the living-docs rule.
- [ ] `CLAUDE.md` Process/Slash-commands/Key-files sections reference
      `docs/working-flows.md`, no longer list `trd.md`/`known_issues.md`, and
      include the "no superpowers/context7" + cross-project rules.
- [ ] `docs/test/testing.md` exists and contains the mock-seam / `--mode mock` /
      contract-test content formerly in `trd.md`.
- [ ] `docs/test/qa-checklist.csv` exists (the former
      `docs/zanto-test-checklist.csv`, kept live — not archived).
- [ ] `trd.md` and `known_issues.md` no longer exist at repo root.
- [ ] `docs/product.md` exists; `docs/vision/`, `docs/product/`, `docs/design/`,
      `docs/stories/` no longer exist.
- [ ] `docs/archive/` contains the former `docs/specs/*`, `docs/plans/*`,
      `docs/superpowers/*`, `docs/reviews/*`, `release-smoke.md`, and the archived
      agent-branches file; `docs/archive/README.md` explains it.
- [ ] `docs/superpowers/`, `docs/plans/`, `docs/reviews/` directories are gone;
      `docs/specs/` exists (empty, with `.gitkeep`).
- [ ] `cargo build` still succeeds (no source touched) and `git status` is clean
      after commit.
- [ ] `grep -rn "trd.md\|known_issues.md" CLAUDE.md .claude/` returns nothing
      (no dangling references).

## Manual test plan

This is a docs/config change — verification is file-state + grep, plus a build
sanity check. Exact commands and expected output:

1. `ls docs/` →
   expected: `architecture  archive  images  product.md  specs  test  working-flows.md`
   (no `vision plans superpowers reviews stories design`; no loose `trd`/issues).
2. `ls docs/test/` → `qa-checklist.csv  testing.md`.
3. `ls docs/specs/` → empty except `.gitkeep`.
4. `test -f trd.md || echo "trd gone"` → `trd gone`;
   `test -f known_issues.md || echo "issues gone"` → `issues gone`.
5. `grep -rn "trd.md\|known_issues.md\|superpowers\|context7" CLAUDE.md docs/working-flows.md`
   → only the *prohibition* lines in working-flows.md mention superpowers/context7;
   no path references to the deleted files.
6. `cargo build` → `Finished` (proves no source/path coupling broke).
7. `git status --porcelain` after commit → empty (clean tree).
