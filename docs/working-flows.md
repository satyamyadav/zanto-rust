# Working flows

The single source of truth for how work is done in this repo. One flow, native
to Claude Code, multi-step and reviewed.

## The flow

Use the project's own slash commands — nothing else:

| Command | What it does | Model |
|---|---|---|
| `/spec <request>` | Write an implementation spec to `docs/specs/YYYY-MM-DD-<slug>.md`. No code. | opus |
| `/execute <spec-path>` | Implement an approved spec, step by step, building + verifying as it goes. | sonnet |
| `/dev <request>` | Full loop: `/spec` → review → `/execute`. | opus (spec), sonnet (impl) |

The flow is **multi-step and reviewed**:

1. **Spec.** Explore the real code first, clarify any ambiguous requirement as a
   concrete question, then write the spec. Never guess around an unknown — ask.
2. **Review.** Present the spec and **wait for explicit approval**. No source
   file is touched until the spec is approved.
3. **Execute.** Implement each step in order, run `cargo build` (and `pnpm check`
   for frontend) after each, and manually verify the changed flow (run the CLI or
   the app) before claiming it works. Report what was confirmed, with evidence.

If a step proves wrong mid-implementation, stop and describe the problem — do not
silently adapt around it.

## Banned tooling

Do **not** use these for this project:

- **superpowers** plugin and its skills (brainstorming, writing-plans,
  subagent-driven-development, executing-plans, etc.). They impose a parallel,
  conflicting flow and write to a separate docs tree. Use `/dev`, `/spec`,
  `/execute` instead.
- **context7** (the docs MCP). Not used here.

These are global Claude Code plugins under `~/.claude/plugins` — they cannot be
removed from within this repo, so the rule is simply: do not invoke them.

## Cross-project rule

This repo stores **no** context, specs, or plans for other repositories (e.g.
`zanto-site`). If work here implies a change in another repo:

- Do **not** create files, specs, or notes for that repo here.
- **Output the requirements** as a self-contained, copy-pasteable block. The
  owner copies it into the other repo, where its own agent picks it up.

Keep this repo's docs about this repo only.

## Docs rule

Living docs (kept current):

- `docs/architecture/*` — the technical reference (overview, modules, data-model,
  permissions, tools, llm, stack-flow).
- `docs/test/*` — testing reference (`testing.md`) + manual QA checklist
  (`qa-checklist.csv`).
- `docs/product.md` — product vision + micro-app architecture + directions.
- `docs/working-flows.md` — this file.

Working docs:

- `docs/specs/` — active/in-progress specs (`YYYY-MM-DD-<slug>.md`). Written by
  `/spec`.

Historical:

- `docs/archive/` — completed/shipped specs, plans, and reviews. Reference only;
  not living docs. Once a spec's work has shipped, it moves here.
