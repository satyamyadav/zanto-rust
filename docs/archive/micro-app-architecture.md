# Design — Micro-App Architecture (desktop)

> **Status: DESIGN / AGREED, not built.** Target client: **Tauri desktop app with
> Svelte, only.** Not the CLI, not any other frontend. The Rust core is likewise
> **focused on the desktop app** — it is the desktop backend, not a general
> multi-client library.
>
> This **revises** the earlier "SubApp Rust trait in core / CLI `--app` surface"
> framing: micro apps live in the **client**, not the core.

## Model: micro-frontends + agentic operation

A **micro app** is a self-contained **Svelte module** mounted in the desktop
shell's **right-side panel**, backed by the generic data + agent capabilities of
`zanto-core` over **Tauri IPC**. Think micro-frontends: each app is independently
defined, mounted, and switched at runtime — but additionally **operable by the
agent through chat**, not only by clicks.

Every app supports two control modalities at once:
- **Manual** — the user clicks: views, controls, and manual flows in the panel.
- **Agentic** — the user chats: the agent operates the app, manipulates its data,
  and can render dynamic views back into the conversation.

## Scope (what this pivot fixes)

- Micro apps are **client-side** (Svelte). They are **not part of core** and do
  **not** need to exist across clients — desktop only.
- **Core is the desktop backend**: data engine, chat loop, permission gate,
  sessions, generic agent tools, generative-UI emission, and the **Tauri IPC**
  surface. No per-app code lives in core.
- The **CLI is no longer an app surface** — it stays a thin harness for exercising
  core primitives during development.

## Desktop shell layout

```
┌──────────────────────────────────────────────────────────────┐
│  zanto desktop (Tauri + Svelte)                                │
│ ┌─────────────────────────┐  ┌─────────────────────────────┐  │
│ │  Chat (threaded)        │  │  Right Panel — mounted app   │  │
│ │  • user/agent turns     │  │  • app views (dynamic)       │  │
│ │  • agent UI blocks (JSON)│  │  • manual flows (buttons)    │  │
│ │    rendered inline       │  │  • data via IPC (fetch-like) │  │
│ └─────────────────────────┘  └─────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
        │                                   │
        │ Tauri IPC (commands + events)     │
        ▼                                   ▼
┌──────────────────────────────────────────────────────────────┐
│  zanto-core (Rust, generic desktop backend)                   │
│   data engine · chat loop · permission gate · sessions ·      │
│   generic agent tools · generative-UI block emission          │
└──────────────────────────────────────────────────────────────┘
```

## What a micro app is composed of

**Frontend (Svelte, per app — lives in the desktop client):**
- **Views** — components mounted in the right panel; created/updated **dynamically**.
- **Manual flows** — user-driven actions (Import, Recategorize…), implemented in TS,
  calling core via IPC.
- **Data access** — fetches from the data engine via **IPC, like HTTP fetch in a
  React/Svelte component**; reactive (`$state`/`$derived`).
- **Agentic profile** — declares the app's **skill** (system-prompt extension), the
  **agent tools** it wants active, and its **data stores**. Pushed to core **on
  mount** so chat operates in the app's context.

**Backend (zanto-core, generic — shared by all apps):**
- **Data engine** — stores, queries, aggregations (deterministic). See
  [the data-layer spec](../specs/2026-06-13-data-layer.md); still valid, only its
  consumers change (IPC + Svelte instead of a Rust trait).
- **Agent tools** — gated, generic data ops + chat.
- **Generative UI** — agent may emit **UI blocks (JSON)** rendered in chat.
- **Tauri IPC** — commands (query stores, run aggregations, set active context, send
  chat) + events (streaming, mount notifications).

## Runtime: mount / switch / active context

- **Single mounted app at a time** in the right panel, plus a **general** mode (none).
- **On mount**: the frontend sends the app's **agentic profile** to core via IPC —
  sets active **skill** + active **tools** + working **stores**. Chat now runs in
  that app's context.
- **On switch / unmount**: revert to general; nothing leaks into general chat.
- Active app id is **persisted on the session**, so reopening restores context.

## Two execution paths (principle preserved, relocated)

- **Agentic** — chat → agent uses the active tools → data engine. **Gated** (HITL).
  The agent can answer with a **UI block** (e.g. a table/metric) rendered in chat.
- **Manual / backend** — right-panel UI → **IPC** → data engine directly.
  **Ungated**. (The Import button runs deterministically; aggregations are computed
  by the data engine, never by the LLM.)

Determinism stays where it matters: numeric aggregation is SQL in the data engine
(or TS in the component), not the model doing arithmetic.

## Generative UI blocks (dynamic views in chat)

Agent replies are artifacts of one of three kinds: **text**, **markdown**, or a
**UI block**. A UI block is **JSON describing a renderable component** that the
Svelte client renders inline:

```jsonc
{ "kind": "ui_block", "block": {
    "type": "table",                  // start small: table | list | card | metric | form
    "title": "Groceries — May",
    "columns": ["merchant", "amount"],
    "rows": [["DMart", 4200], ["BigBasket", 1800]]
}}
```

Start with a **minimal block vocabulary** (table, list, card, metric, maybe form)
and extend. This is the scoped, desktop-native version of the GenUI-D declarative-UI
idea ([vision](../vision/genui-d.md)) — no AG-UI wire protocol, no CDN; just a JSON
contract between core and the Svelte renderer.

## Data access like HTTP fetch

Core exposes the data engine as **Tauri IPC commands**. Svelte components call them
the way a React component calls `fetch()` — request → response, fed into reactive
state. The same commands back both the right-panel views and (wrapped as gated
tools) the agent. One data engine, two callers.

## The core ↔ frontend boundary (the key decision)

| Lives in **core** (Rust, generic) | Lives in **frontend** (Svelte, per app) |
|---|---|
| Data engine (stores, queries, aggregations) | App views / dynamic UI (right panel) |
| Chat loop, permission gate, sessions | Manual flows (TS, call IPC) |
| Generic agent tools (gated data ops) | App registry + mount/switch lifecycle |
| Generative-UI block emission | Agentic-profile declaration (skill/tools/stores) |
| Tauri IPC surface | UI-block rendering |

**Apps are not in core; core is parameterized at runtime by the active app's profile
pushed from the frontend.**

> **Open item to confirm:** per-app **deterministic logic** (e.g. a custom report
> calc) is assumed to live in **frontend TS**, with core offering only *generic*
> deterministic data ops (query/aggregate). If a heavy or shared deterministic op
> emerges, it can be promoted to a generic core capability + IPC command. Flag if
> you'd rather keep a per-app Rust backend slot.

## Finance as the first micro app

- **Svelte module** (right panel): monthly dashboard, transactions table, **Import**
  button (manual flow), category editor.
- **Agentic profile**: skill ("you manage the user's finances…"), tools
  (`add_transaction`, `query_transactions`, `run_report`), stores (transactions,
  categories, budgets).
- **Data via IPC**: the dashboard fetches a monthly aggregation from the data engine
  (deterministic).
- **Agentic example**: "spend on groceries in May?" → agent queries → replies with a
  **UI block** (metric + table) in chat.

A second app (e.g. project hours) is another Svelte module + its profile — **zero
core changes**.

## Build order (desktop-first)

1. **Data engine** in core + **Tauri IPC** for it (query/aggregate). *Spec drafted.*
2. **Generative-UI block protocol** — JSON schema, agent emission, minimal renderer
   contract.
3. **Tauri + Svelte shell** — chat panel + right app panel + mount/switch +
   active-context IPC (skill/tools/stores).
4. **Finance micro app** (Svelte) — profile + manual flows + views.
5. **Input parsers** — CSV first (core generic command or frontend TS).
6. **More block types / apps.**

The CLI is not in this path; it remains a core-primitive test tool.

## Deferred / open

- **User-authored apps** — now a frontend concern (drop-in Svelte modules); convention
  comes later.
- **Multi-active** — rejected (single mounted + general).
- **Per-app deterministic logic location** — confirm frontend-TS vs promotable core
  capability (see open item above).
- **UI-block vocabulary** — start minimal, grow on demand.
- **Data-store registry location** — carried in the data-layer spec (DB meta-table
  vs `.zanto/` config).
