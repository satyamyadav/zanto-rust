# A4 — Context sources + skills/preprompt loader

- **Date:** 2026-06-17
- **Wave:** A (core foundations), batch 2 (after A1; also after A2 for `system_info`)
- **Owner of:** new `crates/zanto-core/src/context.rs`, `chat.rs` system-prompt builder

## Summary
Let the user point the agent at **context sources** (files/dirs read into every turn)
and author **skills/preprompts** as markdown (`.zanto/skills/*.md`) that can be
selected to steer a session. Core loads these and injects them — plus the A2
**system-info** block — into the system prompt. (Config fields come from A1; the
pick-a-skill UI is a later wave.)

## Affected files
- `crates/zanto-core/src/lib.rs` — `pub mod context;`
- `crates/zanto-core/src/context.rs` — loaders.
- `crates/zanto-core/src/chat.rs` — system-prompt assembly; `ChatConfig.context`.

## Design

### Loaders (`context.rs`)
```rust
pub struct Skill { pub name: String, pub body: String }   // name = file stem

/// Concatenate the user's context sources into one prompt block.
/// Files read directly; dirs → top-level *.md/*.txt only, each capped; total capped
/// (e.g. 32 KB) with a truncation note. Missing paths skipped (warn-log, no error).
pub fn load_context(sources: &[String]) -> String;

/// Discover skills in `<project_dir>/.zanto/skills/*.md` (and global skills dir).
pub fn list_skills(project_dir: Option<&Path>) -> Vec<Skill>;
pub fn get_skill(project_dir: Option<&Path>, name: &str) -> Option<Skill>;
```
- Each loaded source is wrapped with a header: `--- context: <path> ---`.
- Caps are constants; document them. No filesystem watching (explicit load per turn).

### chat.rs system-prompt assembly
Current builder concatenates `base_prompt` + optional `skill`. Extend to compose, in
order:
1. `base_prompt`
2. `session::system_info()` (from A2) — `--- system ---`
3. `config.context` (if `Some`) — the loaded context block
4. `config.skill` (the active app skill, unchanged) and/or a selected preprompt body

Add `pub context: Option<String>` to `ChatConfig`; `ChatConfig::new` sets `None`.
Provide a helper `build_system_prompt(base, system_info, context, skill) -> String`
that is unit-testable.

## Acceptance checks
- `cargo build` clean; existing tests pass.
- New tests: `load_context` over a tempdir (file + dir with .md/.txt; respects cap;
  skips missing); `list_skills`/`get_skill` over a temp `.zanto/skills`;
  `build_system_prompt` includes system-info + context + skill in order.

## Notes / handoff
- A1 must have added `context_sources` + `project_dir` to `Settings` first.
- C7 (@-tag / slash) and B2/B3 surface skill selection in the UI; A4 only provides the
  load + inject primitives and the `ChatConfig.context` channel.
- The desktop wires `config.context = Some(load_context(settings.context_sources))`
  and an optional selected skill in B1/B3 (not this unit).
