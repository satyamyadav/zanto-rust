# Backlog

Deferred features and code-quality cleanups. Bugs live in
[known_issues.md](../known_issues.md). Items below were deferred from the
2026-06-19 dynamic-genai-provider-settings work (see `docs/specs/`).

## Features

### genai surface (continuation of A/B/C)

- **D — Custom OpenAI-compatible endpoint providers.** Add-your-own provider by
  name + base URL + key. The `Provider(AdapterKind)` newtype + registry leaves
  this easy (genai's OpenAI adapter works against any base URL). Was an explicit
  v1 non-goal.
- **Per-provider generation-parameter overrides.** `GenerationParams` is global
  in v1; allow each provider to carry its own (different sweet spots; some knobs
  like `reasoning_effort` only apply to certain providers).
- **Generation UI: `stop_sequences` + `extra_body`.** Both exist in the backend
  `GenerationParams` and round-trip through IPC, but the Settings "Generation"
  section does not edit them yet (the `extra_body` advanced-JSON escape hatch and
  a stop-sequence list were scoped out of the Task 7 UI).
- **Generation UI: more `ChatOptions`.** Surface `tool_choice`,
  `response_format` (JSON-schema / structured output), `service_tier`,
  `cache_control` — the rest of genai's tuning surface that the spec listed as
  out of scope.
- **`reasoning_effort`: expose `Max` and `Budget(n)`.** The UI offers
  `none|minimal|low|medium|high|xhigh`; genai also has `Max` and `Budget(u32)`.

### Platform / infra

- **App-level secret-store fallback when the OS keychain is unavailable.** On
  Linux without a Secret Service, keychain writes fail (the app currently falls
  back to reading env vars only). Consider an encrypted-file store in the config
  dir. Has a security tradeoff — needs a design decision. (Today's mitigation:
  set `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` / `GEMINI_API_KEY` in the launch
  env; the app reads them and reports `has_key`.)

## Code-quality cleanups

- **`config.rs` `GenerationParams::overlay` / `merge`.** Verbose per-field
  `if other.x.is_some()`; could use `self.x = other.x.or(self.x)`. The
  `std::mem::take` in the by-value `merge` is unnecessary. `Settings.project_dir`
  is `Option<String>` converted to `PathBuf` at three call sites — consider
  storing `Option<PathBuf>` (it's already canonicalized at load).
- **`ToolService::new` footgun.** It now delegates to
  `with_project_dir(.., None)` with no `#[deprecated]` pointer; a future caller
  using `new` silently loses project-scoped artifacts. Add a doc/deprecation
  note steering callers to `with_project_dir`.
- **`refreshModels` swallows the error.** `SettingsDialog.svelte`'s catch shows a
  friendly hint but discards `e`; add `console.error(e)` so genuine failures
  (bad endpoint) are diagnosable.
