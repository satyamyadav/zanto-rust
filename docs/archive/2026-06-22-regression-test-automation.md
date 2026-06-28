# Spec: automate the regression checklist (R-1..R-9)

**Date:** 2026-06-22
**Scope:** Convert the 9 Regression rows of `docs/zanto-test-checklist.csv` into automated tests, using the split-bridge harness already on `main`. UI-behavior rows become Playwright specs against an expanded mock backend; logic rows become Rust `cargo test`s. Also annotate the CSV so each row records whether/how it is automated. Desktop client only.

## Background

The checklist's Regression rows (R-1..R-9, added 2026-06-18) guard fixes made that day. They are currently manual. This spec automates them so the fixes can't silently regress, and establishes the patterns (scenario-driven streaming, component-block assertions, CSV provenance) that later batches reuse.

## Layer assignment

| Row | What it guards | Layer | Test |
|---|---|---|---|
| R-1 | Chart renders inline via ApexCharts in ONE step; no base64-image / markdown-table fallback | UI | Playwright |
| R-2 | `render_artifact`/`chart` tool-call card is hidden when it renders as a block | UI | Playwright |
| R-3 | Empty-stop 'Stopped' marker shows live AND survives session reopen | UI | Playwright |
| R-4 | Selected skill persists across restart | Core | Rust (`config.rs` Settings round-trip) |
| R-5 | Finance string amount (`'$12.50'`) coerced to a number, not 0 | Core | Rust (`data/mod.rs`) |
| R-6 | `monthly_summary` renders inline; no tool-call card | UI | Playwright |
| R-7 | Chart pin round-trips (render → pin → reopen from Pinned views → re-render) | UI | Playwright |
| R-8 | `/clear` is deterministic (clears when content; silent no-op when empty; never deadlocks) | UI | Playwright |
| R-9 | App-switch shows a 'Switching…' state; threads don't attach to the wrong app | UI | Playwright |

UI specs: R-1, R-2, R-3, R-6, R-7, R-8, R-9 (7). Rust: R-4, R-5 (2).

## New mock infrastructure (the enabling work)

The current `send_message` mock replays one fixed event script (`send_message.json`). The regression specs need different streamed responses per scenario. Add a **scenario router**:

- New file `crates/zanto-desktop/src/lib/mock/scenarios.ts`: a map of named scenarios, each `{ trigger: string, events: ChatEvent[], response: ChatTurn }`, where `events[]` are `{ event, payload }` entries emitted in order. Scenarios needed:
  - `default` — current "Hi there." markdown stream (unchanged behavior).
  - `chart` — emits a `chat_block` of `{ kind: "component", component_id: "chart", data: {...}, target: "inline" }` then `chat_done`. No base64 image, no markdown table. (Drives R-1.)
  - `chart_with_toolcall` — emits a `chat_tool_call` for `render_artifact` (with `renders_as_block: true` semantics) AND the `chat_block`, then `chat_done`. (Drives R-2 — assert the tool-call card is hidden, the block shows.)
  - `finance_summary` — emits a `chat_block` of `{ kind: "component", component_id: "monthly_summary", data: {...}, target: "inline" }` (+ optional tool_call) then `chat_done`. (Drives R-6.)
  - `silent_stop` — emits NO text events; relies on `interrupt_turn` to produce `chat_stopped` + `chat_done`. (Drives R-3 live half.)
- `send_message` selects the scenario by matching the incoming `text` arg against each scenario's `trigger` (case-insensitive substring), falling back to `default`. The current `send_message.json` fixture remains the `default` scenario's source (keep the contract test intact).
- `interrupt_turn` keeps its existing `interrupted` flag behavior (already reset in `resetBackend`).

Other mock handlers to add (typed to the ipc return types, seeded from fixtures):
- `list_pinned_artifacts`, `read_pinned_artifact`, `pin_artifact_cmd` — for R-7. `pin_artifact_cmd` records a pinned entry in mutable in-memory state and returns an id; `list_pinned_artifacts` returns it; `read_pinned_artifact` returns it by id. `resetBackend` clears this state.
- `query_app` / `run_app_action` — for R-6, return the `monthly_summary` component payload when the finance app queries. Minimal: return canned data shaped for the `monthly_summary` block.
- `mount_app` / `unmount_app` — already return `undefined`; for R-9 keep as-is but ensure `list_apps` returns at least two apps (`chat` + `finance`) so a switch is possible.

Fixtures to add under `contract/fixtures/` (each also gets a Rust contract `#[test]` so its DTO shape stays honest):
- `list_pinned_artifacts.json` (one `PinnedArtifact` element).
- `read_pinned_artifact.json`.
- `pin_artifact_cmd.json` (response: a number id).
- `load_session.json` — a `RenderMsg[]` including one assistant turn with `stopped: true` and minimal/empty segments (drives R-3's reopen half).
- `list_apps.json` — extend to include a `finance` app alongside `chat` (drives R-9). (Update the existing fixture; the contract test still validates it.)

> Scenario event-scripts are mock-internal test scaffolding, NOT contract fixtures — they live in `scenarios.ts`. Contract fixtures remain golden DTO samples validated by `contract.rs`. Keep the two concerns separate.

## UI specs (Playwright) — one file per concern under `tests/ui/`

Each spec sends a trigger phrase (matching a scenario) and asserts on the rendered DOM. Selector discovery is the implementer's job (read the real components; do not change app code). Required assertions:

- `tests/ui/regression-artifacts.spec.ts`
  - **R-1:** send a chart trigger → an ApexCharts chart node renders inline; assert NO `img[src^="data:image/png;base64"]` and NO markdown table (`table`) appears as the artifact. (One block, one step.)
  - **R-2:** send the `chart_with_toolcall` trigger → the chart block renders AND the `render_artifact`/`chart` tool-call card is NOT visible (the artifact-rendering tool call is hidden).
  - **R-7:** render a chart → click the Pin affordance on the rendered view → open Artifacts → Pinned views → assert the chart re-renders from the pinned record.
- `tests/ui/regression-chat.spec.ts`
  - **R-3:** send the `silent_stop` trigger → click Stop before any text → assert the 'Stopped' marker is visible. Then reopen the session (the mock `load_session` returns the stopped turn) → assert the marker still shows.
  - **R-8:** type `/clear` with content in the composer → composer clears; type `/clear` on an empty composer → no-op, no hang/deadlock (the slash menu stays usable).
- `tests/ui/regression-finance.spec.ts`
  - **R-6:** mount the finance app, trigger a `monthly_summary` → the summary renders inline as a component block; assert NO tool-call card above it.
- `tests/ui/regression-app-switch.spec.ts`
  - **R-9:** with `chat` + `finance` apps present, switch between them → assert a 'Switching…' indicator appears during the switch (or, if timing makes that flaky, assert the post-switch app is active and the session list belongs to the switched-to app, not the previous one). Prefer the deterministic post-state assertion; only assert the transient spinner if it can be observed without arbitrary waits.

## Rust tests (`cargo test`)

- **R-4** — in `crates/zanto-core/src/config.rs` tests: assert that a `Settings` with `selected_skill = Some("x")` survives a save→load round-trip (the persistence the manual test exercised via app restart). Use the module's existing test patterns/helpers.
- **R-5** — in `crates/zanto-core/src/data/mod.rs` tests: assert that adding a transaction whose amount is the string `"$12.50"` (or `"12.50"`) stores a numeric `12.50`, not `0`, and that a monthly total includes it. Locate the add-transaction/coercion path and test it directly.

> If a focused test for R-4 or R-5 already exists in those modules, extend/assert rather than duplicate.

## CSV annotation

Add a trailing column `Automation` to `docs/zanto-test-checklist.csv`. Populate it for:
- Each automated regression row → the test that covers it, e.g. `auto: tests/ui/regression-artifacts.spec.ts (R-1)` or `auto: core config.rs (R-4)`.
- Every Layer-C row (S1, S2, W-1..W-4, FB-3, FG-4, DOC-4, CO-1, CO-3, FLOW-1..8) → `manual-only: <reason>` (real Tauri runtime / OS / live model / keychain — not coverable by the mock harness).
Leave other rows blank for now (future batches fill them). Preserve the existing CSV structure/quoting; only append the column.

## Out of scope
- Layer-A and Layer-B rows beyond R-1..R-9 (future batches).
- Real-Tauri E2E for the Layer-C rows.
- Any change to app/runtime code (the only production-code touch allowed is additive mock files + fixtures + tests + the CSV).

## Files

**New:** `src/lib/mock/scenarios.ts`; `tests/ui/regression-{artifacts,chat,finance,app-switch}.spec.ts`; `contract/fixtures/{list_pinned_artifacts,read_pinned_artifact,pin_artifact_cmd,load_session}.json`.
**Modified:** `src/lib/mock/backend.ts` (scenario router + new handlers + mutable pinned state); `contract/fixtures/list_apps.json` (+finance app); `src-tauri/tests/contract.rs` (new `#[test]`s); `crates/zanto-core/src/config.rs` + `src/data/mod.rs` (R-4/R-5 tests); `docs/zanto-test-checklist.csv` (Automation column).

## Success criteria
- `pnpm test:ui` passes including the 7 new regression specs.
- `cargo test` passes including R-4, R-5, and the new contract `#[test]`s.
- `pnpm check` clean.
- The CSV's Automation column marks R-1..R-9 as automated (pointing at their tests) and all Layer-C rows as manual-only.
- No app/runtime code changed; production `tauri dev`/`build` unaffected.
