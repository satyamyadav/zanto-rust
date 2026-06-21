# Changelog

All notable changes to zanto are documented here. Format loosely follows
[Keep a Changelog](https://keepachangelog.com); versions follow [SemVer](https://semver.org).

## [1.0.0] — 2026-06-21

First public release. An early, **unsigned** build for macOS, Windows, and Linux.

### Highlights

- **Local-first AI workspace.** Desktop app (Tauri) + CLI, built in Rust.
- **Bring your own model.** Provider API keys stored in the OS keychain (or read
  from env vars), or run fully offline against local **Ollama**.
- **10+ providers, dynamic settings.** Anthropic, OpenAI, Gemini, Groq, xAI,
  DeepSeek, Together, Fireworks, Cohere, Ollama — registry-driven provider/model
  selection with live model lists.
- **Generation parameters.** Temperature, max tokens, top-p, seed, reasoning
  effort (incl. max / token budget), stop sequences, JSON mode, tool choice, and
  an `extra_body` escape hatch — set globally or overridden per provider.
- **Consented tools.** Filesystem read/write/search/edit, shell, web fetch, and
  document parsing (PDF/Office), each gated by a per-path permission prompt
  (allow once / session / forever / deny).
- **Artifacts.** Render charts, tables, metrics, and markdown inline or on a side
  canvas; store and pin artifacts.
- **Sessions.** SQLite-backed, crash-safe, resumable, with automatic context
  summarization for long conversations; context sources and markdown skills.

### Known limitations

- Builds are **not signed or notarized** — macOS Gatekeeper and Windows
  SmartScreen will warn on first run (see the README install notes).
- **No auto-update** — upgrade by downloading a new release.
- Not yet on any app store.

[1.0.0]: https://github.com/satyamyadav/zanto-rust/releases/tag/v1.0.0
