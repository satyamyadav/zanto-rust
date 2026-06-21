# Backlog

Deferred features and code-quality cleanups. Bugs live in
[known_issues.md](../known_issues.md). Items originated from the 2026-06-19
dynamic-genai-provider-settings work (see `docs/specs/`).

## Features

### Open

- **D — Custom OpenAI-compatible endpoint providers.** Add-your-own provider by
  name + base URL + key. *Deferred by decision* — expected to be largely handled
  by the genai crate; revisit only if a settings-UI mapping is needed.
- **Generation UI: remaining `ChatOptions`.** Still not surfaced:
  `response_format` (full JSON-schema / structured output), `service_tier`,
  `cache_control`. Provider-specific and niche; deferred from the "pragmatic set".
- **App-level secret-store fallback when the OS keychain is unavailable.**
  *Deferred by decision* — users install a Secret Service (gnome-keyring /
  KWallet) or set `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` / `GEMINI_API_KEY` in the
  launch env (the app reads them and reports `has_key`). The keychain-unavailable
  error now points users to the env-var path.

### Done

- ~~Per-provider generation-parameter overrides~~ — `ProviderConfig.generation` +
  `Settings::effective_generation()` (global overlaid with the active provider);
  per-provider override editor in Settings.
- ~~Generation UI: `stop_sequences` + `extra_body`~~ — both editable in the
  `GenerationFields` component (newline list + validated JSON).
- ~~Generation UI: `tool_choice` + `json_mode`~~ — `tool_choice` (auto/none/
  required) and a force-JSON toggle, wired into `ChatOptions`.
- ~~`reasoning_effort`: `Max` and `Budget(n)`~~ — `max` keyword + a token-budget
  input (stored as the numeric string → `ReasoningEffort::Budget`).

## Code-quality cleanups

### Done

- ~~`GenerationParams::overlay` / `Settings::merge`~~ — `overlay` uses
  `Option::or`; `merge` dropped the needless `mem::take`.
- ~~`Settings.project_dir` → `PathBuf` duplication~~ — centralized in
  `Settings::project_dir_path()` (CLI + desktop use it).
- ~~`ToolService::new` footgun~~ — documented that it has no project scope and
  steers callers to `with_project_dir`.
- ~~`refreshModels` swallowed the error~~ — now `console.error`s it before the
  manual-entry hint.

(`Settings.project_dir` remains `Option<String>` for serialization stability;
the `project_dir_path()` helper covers the conversion sites.)
