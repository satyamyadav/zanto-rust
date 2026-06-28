# Testing

How zanto-desktop is tested: the single Tauri seam, mock mode, the thin-command
guardrail, and the golden-fixture contract suite. (See `qa-checklist.csv` in this
folder for the manual QA checklist.)

## The single Tauri seam

All Tauri IPC calls are centralized in `crates/zanto-desktop/src/lib/ipc.ts`. It
re-exports a single `ipc` object whose methods call `invoke` (commands) and
`listen` (events) from the real Tauri APIs. No other `.ts` or `.svelte` file in
`src/` may import from `@tauri-apps/*` directly — a Playwright test
(`tests/ui/seam.spec.ts`) walks the source tree and fails if any stray import
exists outside `lib/ipc.ts` and `lib/mock/`.

## Mock mode (`--mode mock`)

Running `pnpm dev:mock` starts Vite with `--mode mock`, which activates an alias
block in `vite.config.js` that redirects the four Tauri modules to in-memory
fakes:

| Real module | Mock |
|---|---|
| `@tauri-apps/api/core` | `src/lib/mock/core.ts` — `invoke` dispatcher |
| `@tauri-apps/api/event` | `src/lib/mock/event.ts` — `listen` + `emit` bus |
| `@tauri-apps/api/webviewWindow` | `src/lib/mock/webviewWindow.ts` |
| `@tauri-apps/plugin-os` | `src/lib/mock/os.ts` |

Because the seam is narrow, the swap is complete: the UI runs in a plain browser
on port 1430 with no Tauri binary involved.

`mock/core.ts` dispatches `invoke(cmd, args)` to a handler table in
`mock/backend.ts`, keyed by the exact command name used in `ipc.ts`. An unknown
command throws immediately so a missing handler surfaces loudly rather than
silently returning `undefined`.

`mock/event.ts` exposes `listen` (registers a handler set per event name) and
`emit` (delivers a payload to all current handlers). The backend's `send_message`
handler iterates over a scripted event sequence from its fixture
(`sendMessageFx.events`), calling `emit` for each entry to replay `chat_chunk`,
`chat_tool_call`, etc., with microtask yields between items so the UI updates
between deltas.

`mock/backend.ts` imports fixture files from `contract/fixtures/*.json` and casts
their `response` field to the corresponding TypeScript type from `ipc.ts`. That
cast is the TypeScript side of the contract: a shape mismatch between fixture and
DTO becomes a compile error caught by `pnpm check`.

## Thin-command guardrail

All business logic lives in `zanto-core`. A `#[tauri::command]` body should only
adapt between the IPC surface and a `zanto-core` call, then emit any streaming
events. This keeps the command thin enough that headless `cargo test` on the core
covers the real behaviour; the contract tests only verify DTO shape.

## Golden fixtures and the Rust contract test

`contract/fixtures/` holds one JSON file per command with the shape:

```json
{ "request": {...}, "response": {...}, "events": [...] }
```

`src-tauri/tests/contract.rs` is a `cargo test` suite that reads each fixture and
deserializes `response` into the real Rust DTO (e.g. `ConfigDto`,
`Vec<AppManifest>`, `ChatTurn`). If the fixture drifts from the Rust type, the
test fails. Response DTOs must derive `serde::Deserialize` to participate.

## Adding a command

1. Add a `#[tauri::command]` function in `src-tauri/src/` and register it in
   `lib.rs`.
2. Add a typed handler to `mock/backend.ts` keyed by the exact command name.
3. Add `contract/fixtures/<command>.json` with `request`, `response`, and (if the
   command streams events) `events`.
4. Add a `#[test]` in `contract.rs` that deserializes
   `fixture("<command>")["response"]` into the response DTO. If the DTO lacks
   `#[derive(Deserialize)]`, add it.

## Commands

```bash
pnpm dev:mock          # UI in browser, all Tauri calls mocked (port 1430)
pnpm test:ui           # Playwright specs (harness, chat, seam guardrail)
pnpm check             # svelte-check: TS types + DTO/fixture shape contract
cargo test             # core unit tests + contract deserialization tests
```

CI runs two headless jobs: `rust` (`cargo test`, including the contract suite)
and `web` (`pnpm check` then `pnpm test:ui`).
