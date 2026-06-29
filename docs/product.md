# Product

What zanto is, the architecture idea behind it, and where it's heading. A living
summary — full source material (the GenUI vision, the finance plan, the proof
story) lives in `docs/archive/`.

## What zanto is

A **local-first, AI-native desktop app**: a Tauri + Svelte shell over a Rust core
(`zanto-core`). You operate it two ways at once — **manually** (click views and
controls) and **agentically** (chat, and the agent operates the app, manipulates
its data, and renders dynamic views back into the conversation).

The wedge is privacy and trust: **your data never leaves your machine.** The
inference model is interchangeable (local Ollama, or a cloud provider) — the
governed execution layer is what you trust. Every mutation passes a
human-in-the-loop permission gate; deterministic Rust does the math, never the
LLM.

### Proof point

A 14B local model on a MacBook Air drove zanto on a *separate* Arch box through a
real system task (updating Chrome via an AUR git package), with a permission gate
on every mutation. The brain and the machine being acted on were different
machines. Full write-up: `docs/archive/2026-06-13-chrome-update-local-model.md`.

## Micro-app architecture

A **micro app** is a self-contained **Svelte module** mounted in the desktop
shell's right-side panel, backed by `zanto-core`'s generic data + agent
capabilities over **Tauri IPC**. Think micro-frontends — each app is independently
defined, mounted, and switched at runtime — but additionally **operable by the
agent through chat**, not only by clicks.

- Micro apps are **client-side** (Svelte). They are not part of core and are
  desktop-only.
- **Core is the desktop backend**: data engine, chat loop, permission gate,
  sessions, generic agent tools, generative-UI emission, and the Tauri IPC
  surface. No per-app code lives in core.

Every app supports both modalities at once: **Manual** (panel views/controls) and
**Agentic** (the agent operates it via chat). Personal Finance is the reference
micro app.

> The longer-term "GenUI-D" vision (a Node/Fastify + Svelte web shell with CDN
> component loading, MCP/AG-UI orchestration, plugin discovery) is parked in
> `docs/archive/genui-d.md` as future intent — it is a *frontend/runtime* layer
> that would sit alongside the Rust core, not a replacement. Not built.

## Directions

Live directions (not committed scope — captured so they're not lost):

**Upcoming app/feature work**
- Token counter; loader at end of message; user chat-bubble redesign.
- New apps: File Manager, Video Editor.
- Skills editor; Svelte/HTML-page artifacts.

**Packaging polish** (deferred from the beta.2 install work — non-blocking):
- Harden `install.sh`'s webkit2gtk-4.1 check — it false-warns "not found" even
  when installed (the `ldconfig -p` grep is fragile under `curl | bash`'s minimal
  PATH; check the actual `/usr/lib*/libwebkit2gtk-4.1.so.0` paths instead).
  (Command stays `zanto-desktop`; binary not renamed — Cargo package unchanged.)
- Consider an AUR `PKGBUILD` for native Arch install (separate packaging handoff).

**Personal Finance — next version.** Today it's a trustworthy *transaction
logger*; the next step is a *finance manager*. The known gaps to close (full plan
archived as `docs/archive/finance-next-version-plan.md`):
1. A real **money model** (income vs expense, transfers, sign convention; use the
   collected `monthly_income`).
2. **Editable ledger** (edit/delete; safe statement re-import without
   double-counting).
3. **Budgets** (budget store, budget-vs-actual, alerts).
4. **Enforced categorization** (validate against profile categories; rules).
5. **Insight layer** (recurring/subscription detection, MoM deltas, trends).
   Constraint to lean into, not fight: no backend, import-driven, local-first.

**Deferred / parked** (from provider-settings work)
- Custom OpenAI-compatible "add your own provider" (name + base URL + key).
- Remaining `ChatOptions` UI (`response_format`/structured output, `service_tier`,
  `cache_control`).
- App-level secret-store fallback when the OS keychain is unavailable (today:
  install a Secret Service or set `*_API_KEY` env vars).
