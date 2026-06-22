# Spec: split-bridge testing for zanto-desktop

**Date:** 2026-06-21
**Scope:** Test the Svelte UI and the Rust (Tauri) backend independently across the
IPC seam, with a shared contract that prevents silent drift. Desktop client only —
the CLI and `zanto-core` library API are out of scope except where already tested.

## Problem

UI and backend communicate only over Tauri IPC (`invoke` commands + `listen`
events). Today neither side is testable without launching the full app: `invoke`
throws outside a Tauri webview, and the command layer needs an `AppHandle` +
`State`. We want fast, headless, automated tests for each side in isolation, plus a
guard so the two mocked halves can't both pass while the integrated app breaks.

## Architecture

The single seam is the Tauri API surface, used only inside
[`src/lib/ipc.ts`](../../crates/zanto-desktop/src/lib/ipc.ts) (`invoke`, `listen`,
`getCurrentWebviewWindow`, `platform`). Everything else imports the typed `ipc`
object. Three parts:

1. **UI track** — run the SvelteKit app in a real headless browser with the Tauri
   API replaced by a mock that dispatches to an in-memory fake backend.
2. **Backend track** — keep behavioral logic in `zanto-core` (already `cargo test`-ed)
   and keep `#[tauri::command]` bodies thin so the command layer needs no runtime test.
3. **Contract** — shared golden fixtures (one JSON per command) consumed by *both*
   the Rust structs (serde round-trip test) and the typed TS mock backend
   (`svelte-check` compile check). Drift on either side fails a test.

```
                 contract/fixtures/<command>.json   ← single source of truth
                      /                       \
   Rust: tests/contract.rs              TS: src/lib/mock/backend.ts
   serde deserialize → DTOs             typed handlers → ipc return types
        (cargo test)                         (svelte-check)
                                                  |
                                         vite --mode mock (alias)
                                                  |
                                       Playwright (headless Chromium)
```

## Part 1 — UI track (real browser, mocked bridge)

### Mock modules
New directory `crates/zanto-desktop/src/lib/mock/`:

- `core.ts` — exports `invoke(cmd, args)`. Dispatches by command name to
  `backend.ts`. Also handles the namespaced plugin call `plugin:dialog|open`
  (used by `ipc.pickFiles`). Unknown command → throws (so missing handlers surface
  loudly in tests).
- `event.ts` — exports `listen(event, cb)` returning an `UnlistenFn`. Backed by a
  tiny in-module event bus so the fake backend can `emit(event, payload)`.
- `webviewWindow.ts` — exports `getCurrentWebviewWindow()` returning a stub whose
  `onDragDropEvent` registers a handler the test can trigger via the bus.
- `os.ts` — exports `platform()` returning a fixed value (default `"linux"`,
  overridable via a query param or global for shortcut-glyph tests).

These mirror exactly the imports in `ipc.ts`; `ipc.ts` itself is **not modified**.

### Vite alias (mock mode only)
In [`vite.config.js`](../../crates/zanto-desktop/vite.config.js), when
`mode === "mock"`, add `resolve.alias` entries mapping:

| Real specifier | Mock |
|---|---|
| `@tauri-apps/api/core` | `src/lib/mock/core.ts` |
| `@tauri-apps/api/event` | `src/lib/mock/event.ts` |
| `@tauri-apps/api/webviewWindow` | `src/lib/mock/webviewWindow.ts` |
| `@tauri-apps/plugin-os` | `src/lib/mock/os.ts` |

The alias is applied **only** under `--mode mock`. `tauri dev` / `tauri build`
resolve the real packages unchanged. The server runs on a dedicated port
(`1430`, `strictPort`) so it never collides with the Tauri dev server (`1420`).

### Fake backend
`src/lib/mock/backend.ts`:

- In-memory state seeded from fixtures: config, sessions list + messages,
  artifacts (stored + pinned), apps/catalogue, skills.
- One handler per `invoke` command (the full surface in `ipc.ts`). Each handler is
  **typed to the corresponding `ipc` return type** (e.g. `getConfig(): Config`),
  sourcing its value from the imported fixture JSON — this typing is the TS half of
  the contract (see Part 3).
- Streaming commands (`send_message`): a handler that emits a scripted event
  sequence over the bus (`chat_chunk` deltas → optional `chat_tool_call` /
  `chat_tool_result` / `chat_block` → `chat_done`, or `chat_stopped` when
  `interrupt_turn` was called) then resolves the returned `ChatTurn`. The script is
  data-driven from a fixture so tests can assert against known content.
- `interaction_request` (HITL): a handler that emits a request and resolves when
  `respond` is called.
- Deterministic: no timers required to resolve; event emission is synchronous-ish
  (microtask) so Playwright can await UI state without arbitrary sleeps.

### Tests
- `@playwright/test` (dev dependency), headless Chromium.
- `crates/zanto-desktop/playwright.config.ts`: `webServer` runs
  `pnpm dev:mock`, `baseURL` the mock port, single Chromium project, CI-friendly
  retries.
- Specs in `crates/zanto-desktop/tests/ui/*.spec.ts`. Initial coverage (happy paths
  that exercise the seam, not exhaustive UI):
  - send a message → streamed assistant turn renders.
  - open/select a session from the sidebar → messages load.
  - settings: change provider/model → `set_config` reflected.
  - a HITL approval round-trip.
  - an artifact pin → appears in the browser.

## Part 2 — Backend track (headless)

- No new Tauri runtime test harness. Behavior is tested in `zanto-core` via the
  existing `cargo test` (extend module tests there as features land).
- **Guardrail (design rule, enforced by review + the seam test below):**
  `#[tauri::command]` functions stay thin — resolve `State`, call into
  `zanto-core`, emit events. Any non-trivial logic moves to core so it is covered
  headlessly. This is documented in `trd.md` alongside the IPC section.
- The command layer's type correctness (arg + return shapes) is covered by the
  contract test (Part 3), not by invoking the commands.

## Part 3 — Contract (shared golden fixtures)

### Fixtures
`crates/zanto-desktop/contract/fixtures/<command>.json`. Each file:

```json
{ "request": { /* command args, camelCase as the UI sends */ },
  "response": { /* return value */ },
  "events": [ /* optional: ordered event payloads for streaming commands */ ] }
```

One file per `invoke` command in `ipc.ts`. `request`/`events` optional for commands
without args / without streaming.

### Rust side — `src-tauri/tests/contract.rs`
- Globs `../contract/fixtures/*.json`.
- For each, deserializes `response` into the command's real return DTO and `request`
  into the command's arg struct(s) via `serde_json::from_value`, asserting success.
- A `request`/`response` that the real Rust types reject → test failure. Maps each
  fixture filename to its DTO via an explicit table in the test (kept next to the
  command registration in `lib.rs` so additions are obvious).
- Runs under plain `cargo test` — no Tauri runtime, no display.

### TS side — typed mock backend
- `backend.ts` imports the same fixture JSON and assigns each to a handler typed to
  the real `ipc` return type. `pnpm check` (`svelte-check`) fails if a fixture shape
  violates the TS type. No codegen, no extra dependency, no runtime validator.
- Because the mock backend and the Rust test consume the *same* JSON, a shape that
  satisfies one type but not the other fails on its side.

### Drift policy
- Adding/changing a command: update `ipc.ts`, the Rust DTO, and the fixture together;
  both test sides keep them honest.
- CI `check` step warns (non-fatal initially) if an `invoke("...")` command name in
  `ipc.ts` has no fixture file — surfaces forgotten contracts without blocking.

## Automation

### package.json scripts (additions)
```
"dev:mock":      "vite dev --mode mock",
"test:ui":       "playwright test",
"test:contract": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json"
```
(`test:contract` is `check` re-aliased for intent; the TS contract is enforced by
the same type check.)

Backend: `cargo test` (workspace) covers core + `contract.rs`.

### Seam guard
A cheap test asserting `@tauri-apps/api` / `@tauri-apps/plugin-os` are imported
**only** from `src/lib/ipc.ts` and `src/lib/mock/`. Implementation: a Playwright/
node test (or a `check`-time script) that greps `src/` and fails on stray imports.
Keeps the single-seam invariant from eroding.

### CI — GitHub Actions (stub, ready to enable)
`.github/workflows/test.yml` with two parallel jobs:

- **rust:** `cargo test` (core unit tests + `contract.rs`).
- **web:** `pnpm install` → `pnpm check` (TS contract) → `npx playwright install
  --with-deps chromium` → `pnpm test:ui`.

No display server, no Tauri toolchain, no `tauri-driver`. Headless throughout.

## Out of scope (YAGNI)
- Real end-to-end tests over the live JS↔Rust boundary (`tauri-driver`/WebDriver).
  The contract test covers shape agreement; real-boundary E2E can be added later
  behind its own spec once the app stabilizes.
- Tauri mock-runtime command tests.
- TS-from-Rust codegen (`ts-rs`/`typeshare`) — rejected to keep the hand-written,
  documented `ipc.ts` as the TS source of truth.
- CLI and any non-desktop client.

## Files

**New**
- `crates/zanto-desktop/src/lib/mock/core.ts`
- `crates/zanto-desktop/src/lib/mock/event.ts`
- `crates/zanto-desktop/src/lib/mock/webviewWindow.ts`
- `crates/zanto-desktop/src/lib/mock/os.ts`
- `crates/zanto-desktop/src/lib/mock/backend.ts`
- `crates/zanto-desktop/contract/fixtures/*.json` (one per command)
- `crates/zanto-desktop/src-tauri/tests/contract.rs`
- `crates/zanto-desktop/playwright.config.ts`
- `crates/zanto-desktop/tests/ui/*.spec.ts`
- `crates/zanto-desktop/tests/seam.test.ts` (or `check`-time grep script)
- `.github/workflows/test.yml`

**Modified**
- `crates/zanto-desktop/vite.config.js` — `--mode mock` alias branch + mock port.
- `crates/zanto-desktop/package.json` — `@playwright/test` dev dep + scripts.
- `trd.md` — document the seam, thin-command guardrail, and how to add a fixture.

## Success criteria
- `pnpm dev:mock` runs the full UI in a browser with no Tauri runtime.
- `pnpm test:ui` passes headlessly against the mock backend.
- `cargo test` passes including `contract.rs`.
- Changing a Rust DTO without updating its fixture fails `cargo test`; changing an
  `ipc.ts` type without updating the fixture fails `pnpm check`.
- Importing `@tauri-apps/api` outside the seam fails the seam guard.
