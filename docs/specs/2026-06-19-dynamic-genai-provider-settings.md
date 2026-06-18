# Dynamic genai provider settings

Date: 2026-06-19
Status: Approved design (pending implementation plan)

## Problem

The Settings panel's provider configuration is hardcoded and cannot grow with
what genai already supports. genai 0.6.4 ships ~18 provider adapters and a rich
per-request tuning surface; zanto exposes 4 providers, a free-text model field,
and no generation parameters. The provider set is duplicated in five places that
must be hand-edited in lockstep:

- `Provider` enum — 4 variants (`crates/zanto-core/src/config.rs:12`)
- `default_providers()` — fixed list + fixed default models (`crates/zanto-desktop/src-tauri/src/ipc/config.rs:8`)
- `parse_provider()` — exhaustive per-variant match (`crates/zanto-desktop/src-tauri/src/ipc/config.rs:22`)
- `providerLabels` map (`crates/zanto-desktop/src/lib/components/SettingsDialog.svelte:164`)
- the model `<Input>` is free text, not a list (`SettingsDialog.svelte:210`)

Additionally, the chat loop builds `ChatOptions::default()` with capture flags
only (`crates/zanto-core/src/chat.rs:332`), discarding genai's tuning surface:
`temperature`, `max_tokens`, `top_p`, `reasoning_effort`, `seed`,
`stop_sequences`, and the `extra_body` escape hatch.

## Goals

- **A — Dynamic provider registry.** genai's `AdapterKind` becomes the single
  source of truth for providers. The UI is driven by a curated registry, not
  hardcoded lists.
- **B — Live model dropdown.** Populate the model field from the provider's real
  model list via `Client::all_model_names`, with manual entry as a fallback.
- **C — Generation parameters.** Surface a global set of `ChatOptions` knobs in
  Settings, wired into the chat loop.

## Non-goals (v1)

- Per-provider generation-parameter overrides (params are global in v1).
- Custom OpenAI-compatible endpoint providers (direction "D" — a later follow-on
  that the newtype model leaves easy).
- UI for `tool_choice` / `response_format` / `service_tier` / `cache_control`.

## Decisions (from brainstorming)

| Question | Decision |
|---|---|
| Provider representation | Newtype `Provider(AdapterKind)` |
| Generation-param scope | Global (one block, applied every turn) |
| Curated provider set | Tested set (~10) |

---

## A — Dynamic provider registry

### Data model

Replace the closed enum with a thin newtype over genai's `AdapterKind`:

```rust
// zanto-core/src/config.rs
pub struct Provider(pub genai::adapter::AdapterKind);
```

- **Serialization.** Custom serde via `as_lower_str()` / `from_lower_str()` so
  settings.json stores stable lowercase ids (`"anthropic"`, `"openai"`,
  `"gemini"`, `"ollama"`, …). genai's `AdapterKind` already derives
  Serialize/Deserialize, but its default repr is the PascalCase variant name; we
  use the lower-str form for stability and to match existing data.
- **`as_str()`** → `self.0.as_lower_str()` (keychain username + id basis).
- **`env_var()`** → `self.0.default_key_env_name()` (returns `None` when no key
  is needed, e.g. Ollama).
- **`needs_key()`** → `self.env_var().is_some()`.
- **`provider_of(model)`** → `AdapterKind::from_model(model).ok().map(Provider)`,
  falling back to `Provider(AdapterKind::Ollama)` to preserve current behavior.

### Curated registry

```rust
pub const SUPPORTED: &[AdapterKind] = &[
    AdapterKind::Anthropic,
    AdapterKind::OpenAI,
    AdapterKind::Gemini,
    AdapterKind::Groq,
    AdapterKind::Xai,
    AdapterKind::DeepSeek,
    AdapterKind::Together,
    AdapterKind::Fireworks,
    AdapterKind::Cohere,
    AdapterKind::Ollama,
];
```

A `provider_registry()` helper returns, for each entry, a descriptor:
`{ id: as_lower_str, label, needs_key, default_endpoint }`. Labels live in one
small map keyed by `AdapterKind` (e.g. Anthropic → "Anthropic", Xai → "xAI").
`default_endpoint` is `Some("http://localhost:11434/")` for Ollama, else `None`.

Adding a provider later = appending one `AdapterKind` to `SUPPORTED` (plus a
label). No other Rust or Svelte edits.

### Default models

`default_providers()` is replaced by a registry-derived default: a small
`AdapterKind → default model` map for the curated set (e.g. Anthropic →
`claude-opus-4-5`, OpenAI → `gpt-4o`, Gemini → `gemini-2.0-flash`, Ollama →
`qwen2.5:14b`, others → a sensible flagship). Providers without a default model
seed with an empty string and rely on the live model dropdown (B).

### Migration

Existing settings.json provider strings:
- `"anthropic"`, `"gemini"`, `"ollama"` already equal `as_lower_str()` → parse
  unchanged.
- The old `Provider::OpenAI` serialized under `#[serde(rename_all = "snake_case")]`
  as **`"open_ai"`**, but `AdapterKind::OpenAI.as_lower_str()` is **`"openai"`**.
  The custom deserializer must remap `"open_ai"` → `Provider(OpenAI)` on load so
  existing configs keep working. (The keychain username already used `as_str()`
  = `"openai"`, so stored keys need no migration.)

Unknown / unsupported ids in a loaded config deserialize to `None` and are
dropped from the active set rather than erroring the whole file.

### IPC

- `get_config` returns the registry descriptors instead of relying on the
  frontend's hardcoded `providerLabels`.
- `parse_provider(s)` → `AdapterKind::from_lower_str(s).map(Provider)`, error on
  `None`. The exhaustive per-variant match is deleted.
- `ProviderDto` gains `label`, `needs_key`, `default_endpoint`.

### Frontend

`SettingsDialog.svelte`:
- Provider `<Select>` iterates the registry from config; `providerLabels` map is
  deleted (labels come from the DTO).
- The Ollama-specific endpoint branch keys off `needs_key === false` /
  `default_endpoint != null` instead of the literal `"ollama"` string.

---

## B — Live model dropdown

### IPC

New command:

```rust
#[tauri::command]
async fn list_models(provider: String) -> Result<Vec<String>, String>
```

- Parse `provider` via `parse_provider`.
- Build a genai `Client` with the **same** auth + endpoint resolution used by the
  chat loop (keychain/env key via the auth resolver; Ollama endpoint override).
  This resolution is currently inline in `chat.rs:235-258`; extract it into a
  shared `config`/client helper so chat and `list_models` share one code path.
- Call `client.all_model_names(kind, provider_config).await`; map errors to
  `Err(String)`.

### Frontend

- The model field becomes a combobox: a dropdown of fetched names **plus**
  free-text entry for custom/unlisted models (preserves today's flexibility).
- A **Refresh** button triggers `list_models`. Results are cached per provider
  in the component for the session.
- Failure (missing key, offline, adapter without a list endpoint) is non-fatal:
  show an inline hint ("Couldn't load models — type the name manually") and keep
  the text field usable.

---

## C — Generation parameters (global)

### Data model

```rust
// zanto-core/src/config.rs — new, added to Settings
pub struct GenerationParams {
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f64>,
    pub reasoning_effort: Option<String>, // none|minimal|low|medium|high|xhigh
    pub seed: Option<u64>,
    pub stop_sequences: Vec<String>,      // empty = none
    pub extra_body: Option<serde_json::Value>, // advanced escape hatch
}
```

Added to `Settings` as `#[serde(default)] pub generation: GenerationParams`
(defaults to all-`None` / empty → behaves exactly as today).

### Wiring

In `chat.rs`, the existing `stream_options` builder is extended from the
resolved `GenerationParams`:

- Each `Some` field maps to its `with_*` builder
  (`with_temperature`, `with_max_tokens`, `with_top_p`, `with_seed`).
- `reasoning_effort` parses via `ReasoningEffort::from_str`-style lookup
  (`none|minimal|low|medium|high|xhigh`); ignored if unparseable.
- `stop_sequences` → `with_stop_sequences` when non-empty.
- `extra_body` → `with_extra_body` when present.
- Capture flags (`with_capture_content` / `_tool_calls` /
  `_reasoning_content`) are preserved.

genai ignores parameters an adapter doesn't support, so always-send-when-set is
safe across the curated providers. Both the main turn and the summarize call may
share the same base options, but summarization keeps its own minimal options to
avoid, e.g., forcing reasoning on a summary pass.

### IPC + Frontend

- `get_config` / `set_config` carry the `generation` block;
  `ConfigDto` / `ConfigPatch` gain it.
- New "Generation" `<section>` in `SettingsDialog.svelte`: numeric inputs for
  temperature / max_tokens / top_p / seed, a `<Select>` for reasoning effort, a
  comma/newline list for stop sequences, and a collapsible **Advanced**
  textarea for `extra_body` JSON (validated on save; invalid JSON shows a toast
  and blocks save of that field only).

---

## Affected files

| File | Change |
|---|---|
| `zanto-core/src/config.rs` | `Provider(AdapterKind)` newtype + serde; `SUPPORTED` + registry + labels + default-model map; `provider_of`/`env_var`/`model_context_window` updated; `GenerationParams` in `Settings`; `open_ai`→`openai` migration |
| `zanto-core/src/chat.rs` | extract shared client/auth/endpoint helper; build `ChatOptions` from `GenerationParams` |
| `zanto-desktop/src-tauri/src/ipc/config.rs` | registry-driven `get_config`; `parse_provider` via `from_lower_str`; carry `generation`; new `list_models` |
| `zanto-desktop/src-tauri/src/ipc/mod.rs` | `ProviderDto` (+label/needs_key/default_endpoint), `ConfigDto`/`ConfigPatch` (+generation), `list_models` registration |
| `zanto-desktop/src/lib/ipc.ts` | type updates + `listModels` |
| `zanto-desktop/src/lib/components/SettingsDialog.svelte` | registry-driven provider select; model combobox + Refresh; Generation section |

`model_context_window` gains a default arm for adapters not in the original four
(e.g. 32k) so the auto context policy still works for new providers.

## Testing

- **Unit (zanto-core):** `Provider` round-trips through serde for each curated
  id; `"open_ai"` legacy string deserializes to `Provider(OpenAI)`; unknown id →
  drop; `GenerationParams` default omits all options; `ChatOptions` builder
  applies only the set fields.
- **Manual (per CLAUDE.md "always run the app"):**
  1. Settings shows ~10 providers from the registry.
  2. Pick a provider with a saved key → Refresh lists real models; pick one;
     save; send a message end-to-end.
  3. Provider without a key → model Refresh degrades gracefully; manual entry
     still works.
  4. Set temperature + reasoning effort → confirm the turn still streams and the
     options are accepted (no provider error).
  5. Load a pre-existing config with `"open_ai"` → it resolves to OpenAI.

## Risks

- **`all_model_names` variance.** Some adapters query a live `/models` endpoint;
  others may return curated/static lists or error without a key. Handled by the
  non-fatal fallback in B.
- **Migration of `open_ai`.** Covered by the remap; called out explicitly so the
  implementation doesn't miss it.
- **Provider-specific param rejection.** Mitigated by genai's per-adapter
  normalization; the `extra_body` escape hatch is the explicit power-user path.
