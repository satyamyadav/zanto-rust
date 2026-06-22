# Regression Test Automation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automate the 9 Regression rows (R-1..R-9) of `docs/zanto-test-checklist.csv` — 7 as Playwright UI specs over an expanded mock backend, 2 as Rust `cargo test`s — and annotate the CSV with automation provenance.

**Architecture:** Extend the existing split-bridge harness. A scenario router in the mock `send_message` streams different responses keyed by the message text, so each spec triggers its case declaratively. New mock handlers + golden fixtures (each contract-tested) support pin/finance/session-reload flows. Rust tests cover the finance amount-coercion and skill-persistence logic directly.

**Tech Stack:** SvelteKit + Svelte 5, Vite 6 (`--mode mock`), `@playwright/test`, Rust + serde + `cargo test`.

## Global Constraints

- Desktop client only. No app/runtime code changes — additive only: `src/lib/mock/*`, `contract/fixtures/*`, `tests/ui/*`, `src-tauri/tests/contract.rs`, Rust test modules, and the CSV.
- `src/lib/ipc.ts` and its consumers/components MUST NOT change. The Vite mock alias applies ONLY under `--mode mock` (port 1430). Production `tauri dev`/`build` unaffected.
- No new JS/Rust dependency.
- Mock command handlers are keyed by the exact `invoke` command name and typed to the `ipc` return type; fixtures are golden DTO samples validated by `contract.rs`. Scenario event-scripts are mock-internal scaffolding in `scenarios.ts`, NOT contract fixtures.
- `send_message` event emission: text via `emit("chat_chunk", { text })`; a component block via `emit("chat_block", { block })` where `block` is a `ChatBlock`; a tool call via `emit("chat_tool_call", { id, name, args })`; end via `emit("chat_done", null)`; interrupt via `emit("chat_stopped", null)` then `emit("chat_done", null)`.
- `ChatBlock` component form: `{ kind: "component", component_id: string, data: any, target: "inline" | "canvas" }`.
- Verify gates after each task that changes them: `cd crates/zanto-desktop && pnpm check` (0 errors), `pnpm test:ui` (all pass), and from repo root `cargo test -p zanto-desktop --test contract` (and full `cargo test` for the Rust-test task).
- Selectors and opaque component `data` shapes are discovered by reading the real components (do NOT change them) — same pattern used to wire the existing chat spec.

---

### Task 1: Mock scenario router + new handlers + fixtures (enabling infra)

**Files:**
- Create: `crates/zanto-desktop/src/lib/mock/scenarios.ts`
- Modify: `crates/zanto-desktop/src/lib/mock/backend.ts`
- Create: `crates/zanto-desktop/contract/fixtures/{list_pinned_artifacts,read_pinned_artifact,pin_artifact_cmd,load_session}.json`
- Modify: `crates/zanto-desktop/contract/fixtures/list_apps.json`
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/artifacts.rs` (add `Deserialize` to `PinnedArtifact`)
- Modify: `crates/zanto-desktop/src-tauri/tests/contract.rs`

**Interfaces:**
- Produces (consumed by Tasks 2–6):
  - `scenarios.ts` → `export type ScenarioEvent = { event: string; payload: unknown }`, `export type Scenario = { trigger: string; events: ScenarioEvent[]; response: ChatTurn }`, `export const scenarios: Scenario[]`, `export const defaultScenario: Scenario`.
  - `backend.ts` → `send_message` routes by `text`; handlers `list_pinned_artifacts`, `read_pinned_artifact`, `pin_artifact_cmd`, `query_app`, `run_app_action`; mutable in-memory `pinned` array reset by `resetBackend()`.
  - `list_apps.json` response includes apps with ids `"chat"` and `"finance"`.

- [ ] **Step 1: Add `Deserialize` to `PinnedArtifact` and write its contract tests (RED)**

In `crates/zanto-desktop/src-tauri/src/ipc/artifacts.rs`, change `PinnedArtifact`'s derive to include `Deserialize` (ensure `use serde::{Deserialize, Serialize};` present):
```rust
#[derive(Serialize, Deserialize)]
pub struct PinnedArtifact {
```
In `crates/zanto-desktop/src-tauri/tests/contract.rs`, add (adjust the `use` import to the crate's existing style):
```rust
#[test]
fn list_pinned_artifacts_response_matches_dto() {
    let fx = fixture("list_pinned_artifacts");
    let _v: Vec<zanto_desktop_lib::ipc::PinnedArtifact> =
        serde_json::from_value(fx["response"].clone()).expect("list_pinned_artifacts → Vec<PinnedArtifact>");
}
#[test]
fn read_pinned_artifact_response_matches_dto() {
    let fx = fixture("read_pinned_artifact");
    let _v: zanto_desktop_lib::ipc::PinnedArtifact =
        serde_json::from_value(fx["response"].clone()).expect("read_pinned_artifact → PinnedArtifact");
}
#[test]
fn pin_artifact_cmd_response_matches_dto() {
    let fx = fixture("pin_artifact_cmd");
    let _v: i64 = serde_json::from_value(fx["response"].clone()).expect("pin_artifact_cmd → i64");
}
#[test]
fn load_session_response_matches_dto() {
    let fx = fixture("load_session");
    let _v: Vec<zanto_desktop_lib::ipc::RenderMsg> =
        serde_json::from_value(fx["response"].clone()).expect("load_session → Vec<RenderMsg>");
}
```

- [ ] **Step 2: Run contract tests — expect FAIL (fixtures missing)**

Run: `cargo test -p zanto-desktop --test contract`
Expected: FAIL — the four new fixture files don't exist yet (panic in `fixture(...)` read).

- [ ] **Step 3: Create the fixtures**

`contract/fixtures/pin_artifact_cmd.json`:
```json
{ "request": { "componentId": "chart", "data": {}, "title": null }, "response": 1 }
```
`contract/fixtures/read_pinned_artifact.json`:
```json
{
  "request": { "id": 1 },
  "response": { "id": 1, "component_id": "chart", "title": "Weekly Values", "target": "inline", "created_at": 1718900000, "data": { "type": "bar", "labels": ["Mon","Tue"], "datasets": [{ "data": [120,200], "label": "Weekly Values" }] } }
}
```
`contract/fixtures/list_pinned_artifacts.json`:
```json
{ "request": {}, "response": [ { "id": 1, "component_id": "chart", "title": "Weekly Values", "target": "inline", "created_at": 1718900000, "data": { "type": "bar", "labels": ["Mon","Tue"], "datasets": [{ "data": [120,200], "label": "Weekly Values" }] } } ] }
```
`contract/fixtures/load_session.json` (an assistant turn that was stopped with no content — drives R-3 reopen; match the `RenderMsg` TS type: `role`, `text`, optional `blocks`, `segments`, `stopped`):
```json
{
  "request": { "id": "sess-test-0001" },
  "response": [
    { "role": "user", "text": "write me a long essay", "blocks": null, "segments": null, "stopped": null },
    { "role": "assistant", "text": "", "blocks": null, "segments": [], "stopped": true }
  ]
}
```

- [ ] **Step 4: Run contract tests — expect PASS**

Run: `cargo test -p zanto-desktop --test contract`
Expected: PASS (all, including the 6 existing). If a fixture shape is rejected, the panic names the field — correct the fixture to match the Rust DTO.

- [ ] **Step 5: Extend `list_apps.json` with a finance app**

Edit `contract/fixtures/list_apps.json` so the `response` array has two entries (keep `chat`, add `finance`). Match the `AppManifest` shape already used by the existing element:
```json
{ "request": {}, "response": [
  { "id": "chat", "name": "Chat", "description": "General chat", "stores": [], "components": [], "start_actions": [] },
  { "id": "finance", "name": "Finance", "description": "Personal finance", "stores": ["transactions"], "components": [], "start_actions": [] }
] }
```
Run `cargo test -p zanto-desktop --test contract` → still PASS.

- [ ] **Step 6: Write the scenario module**

Create `crates/zanto-desktop/src/lib/mock/scenarios.ts`:
```ts
import type { ChatTurn } from "$lib/ipc";

export type ScenarioEvent = { event: string; payload: unknown };
export type Scenario = { trigger: string; events: ScenarioEvent[]; response: ChatTurn };

const chartBlock = {
  kind: "component",
  component_id: "chart",
  // ApexCharts schema confirmed working in the checklist (R-1). Align with Chart.svelte if it expects a different shape.
  data: { type: "bar", labels: ["Mon", "Tue", "Wed"], datasets: [{ data: [120, 200, 150], label: "Weekly Values" }] },
  target: "inline",
};

const summaryBlock = {
  kind: "component",
  component_id: "monthly_summary",
  // Shape to match monthly_summary.svelte — discover the real fields and adjust.
  data: { income: 2000, spent: 12.5, net: 1987.5, by_category: { dining: 12.5 } },
  target: "inline",
};

// Default: plain markdown stream (mirrors the original send_message.json behavior).
export const defaultScenario: Scenario = {
  trigger: "",
  events: [
    { event: "chat_chunk", payload: { text: "Hi " } },
    { event: "chat_chunk", payload: { text: "there." } },
    { event: "chat_done", payload: null },
  ],
  response: { blocks: [{ kind: "markdown", text: "Hi there." }] },
};

export const scenarios: Scenario[] = [
  { trigger: "chart with toolcall", events: [
      { event: "chat_tool_call", payload: { id: "t1", name: "render_artifact", args: { id: "chart", target: "inline" } } },
      { event: "chat_block", payload: { block: chartBlock } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [chartBlock as any] } },
  { trigger: "chart", events: [
      { event: "chat_block", payload: { block: chartBlock } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [chartBlock as any] } },
  { trigger: "finance summary", events: [
      { event: "chat_block", payload: { block: summaryBlock } },
      { event: "chat_done", payload: null },
    ], response: { blocks: [summaryBlock as any] } },
  { trigger: "silent stop", events: [], response: { blocks: [] } },
];

/** Pick the first scenario whose trigger is a case-insensitive substring of the message, else default. */
export function pickScenario(text: string): Scenario {
  const t = text.toLowerCase();
  return scenarios.find((s) => s.trigger && t.includes(s.trigger)) ?? defaultScenario;
}
```

- [ ] **Step 7: Rewire `send_message` and add handlers in `backend.ts`**

In `crates/zanto-desktop/src/lib/mock/backend.ts`: import `pickScenario` (and remove the now-unused `sendMessageFx` import if the default scenario fully replaces it — keep `send_message.json` fixture file on disk for the contract test). Replace the `send_message` handler and add the new handlers. Keep the `interrupted` flag + `resetBackend` reset:
```ts
import { pickScenario } from "./scenarios";
import listPinnedFx from "../../../contract/fixtures/list_pinned_artifacts.json";
import loadSessionFx from "../../../contract/fixtures/load_session.json";

let pinned: any[] = listPinnedFx.response.slice();
let nextPinId = pinned.length + 1;

backend.send_message = async (args: { text?: string }): Promise<ChatTurn> => {
  interrupted = false;
  const sc = pickScenario(args?.text ?? "");
  for (const ev of sc.events) {
    if (interrupted) break;
    emit(ev.event, ev.payload);
    await Promise.resolve();
  }
  if (interrupted) { emit("chat_stopped", null); emit("chat_done", null); }
  return sc.response;
};

backend.load_session = async (): Promise<any> => loadSessionFx.response;
backend.load_session_page = async (): Promise<any> => loadSessionFx.response;
backend.list_pinned_artifacts = async (): Promise<any> => pinned;
backend.read_pinned_artifact = async (a: { id: number }): Promise<any> =>
  pinned.find((p) => p.id === a.id) ?? pinned[0];
backend.pin_artifact_cmd = async (a: { componentId: string; data: any; title?: string }): Promise<number> => {
  const id = nextPinId++;
  pinned.push({ id, component_id: a.componentId, title: a.title ?? null, target: "inline", created_at: 1718900000, data: a.data });
  return id;
};
backend.query_app = async (): Promise<any> => ({ income: 2000, spent: 12.5, net: 1987.5, by_category: { dining: 12.5 } });
backend.run_app_action = async (): Promise<any> => ({});
```
In `resetBackend()` add: `pinned = listPinnedFx.response.slice(); nextPinId = pinned.length + 1;` (alongside the existing `interrupted = false;`).

> If `load_session` already had a handler from earlier work, replace it; ensure the booting app still works (the Sidebar/session reopen now returns the stopped-turn fixture).

- [ ] **Step 8: Verify gates**

Run: `cd crates/zanto-desktop && pnpm check` → 0 errors. Then `pnpm test:ui` → the existing 3 specs (harness, chat, seam) still pass (the default scenario preserves "Hi there."). Then repo root `cargo test -p zanto-desktop --test contract` → all pass.

- [ ] **Step 9: Commit**

```bash
git add crates/zanto-desktop/src/lib/mock/scenarios.ts crates/zanto-desktop/src/lib/mock/backend.ts \
  crates/zanto-desktop/contract/fixtures crates/zanto-desktop/src-tauri/src/ipc/artifacts.rs \
  crates/zanto-desktop/src-tauri/tests/contract.rs
git commit -m "test(desktop): mock scenario router + pin/finance/session handlers + fixtures"
```

---

### Task 2: R-1 + R-2 — artifact rendering specs

**Files:**
- Create: `crates/zanto-desktop/tests/ui/regression-artifacts.spec.ts`

**Interfaces:**
- Consumes: scenarios `chart` (trigger "chart") and `chart with toolcall` (trigger "chart with toolcall") from Task 1.

- [ ] **Step 1: Write the specs (template — discover selectors from the real components)**

Create `tests/ui/regression-artifacts.spec.ts`. Read `src/lib/blocks/Chart.svelte`, `src/lib/Block.svelte`, and `src/lib/components/segments/ToolCallSegment.svelte` to find the real chart node selector and how an artifact-rendering tool call is hidden. Assertions:
```ts
import { test, expect } from "@playwright/test";

test("R-1: chart renders inline in one step, no base64 image or markdown-table fallback", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("show me a chart");
  await composer.press("Enter");
  // ApexCharts mounts a .apexcharts-canvas node (confirm the real selector).
  await expect(page.locator(".apexcharts-canvas")).toBeVisible();
  // No base64 image fallback, no markdown table standing in for the chart.
  await expect(page.locator('img[src^="data:image/png;base64"]')).toHaveCount(0);
});

test("R-2: artifact-rendering tool-call card is hidden when it renders as a block", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("chart with toolcall");
  await composer.press("Enter");
  await expect(page.locator(".apexcharts-canvas")).toBeVisible();
  // The render_artifact tool-call card must NOT be shown above the block.
  await expect(page.getByText("render_artifact")).toHaveCount(0);
});
```

- [ ] **Step 2: Run — iterate selectors until green**

Run: `cd crates/zanto-desktop && pnpm test:ui -- tests/ui/regression-artifacts.spec.ts`
Expected: initially may FAIL on selector mismatch; inspect the real components and adjust the locators (NOT the app). Green when the chart renders and the tool-call card is absent. If the chart block's `data` shape is wrong (chart doesn't mount), fix the `chartBlock.data` in `scenarios.ts` to match `Chart.svelte`.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/regression-artifacts.spec.ts crates/zanto-desktop/src/lib/mock/scenarios.ts
git commit -m "test(desktop): R-1/R-2 chart renders inline, artifact tool-call hidden"
```

---

### Task 3: R-7 — chart pin round-trip

**Files:**
- Modify: `crates/zanto-desktop/tests/ui/regression-artifacts.spec.ts`

**Interfaces:**
- Consumes: `chart` scenario; `pin_artifact_cmd` / `list_pinned_artifacts` / `read_pinned_artifact` handlers from Task 1.

- [ ] **Step 1: Add the pin round-trip spec (discover the pin + Pinned-views UI)**

Read `src/lib/Block.svelte` (the A-5 Pin affordance on view artifacts), `src/lib/components/ArtifactBrowser.svelte`, and the Sidebar Artifacts entry to find: the Pin button, how to open the Artifacts browser, and the Pinned views tab. Append:
```ts
test("R-7: a pinned chart re-renders from the stored record", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("show me a chart");
  await composer.press("Enter");
  await expect(page.locator(".apexcharts-canvas")).toBeVisible();
  // Pin the rendered view (discover the Pin control), open Artifacts → Pinned views,
  // and assert the chart re-renders from the pinned record.
  // ... interactions discovered from the real UI ...
  // Final assertion:
  await expect(page.locator(".apexcharts-canvas")).toBeVisible();
});
```

- [ ] **Step 2: Run — iterate until green**

Run: `cd crates/zanto-desktop && pnpm test:ui -- tests/ui/regression-artifacts.spec.ts`
Expected: PASS for all three (R-1, R-2, R-7). The mock `pin_artifact_cmd` records the pin and `list_pinned_artifacts`/`read_pinned_artifact` return it so the Pinned-views tab re-renders. If the pin flow needs a handler that's missing, note it; the Task-1 handlers should cover it.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/regression-artifacts.spec.ts
git commit -m "test(desktop): R-7 chart pin round-trip"
```

---

### Task 4: R-3 + R-8 — chat stop-marker & /clear specs

**Files:**
- Create: `crates/zanto-desktop/tests/ui/regression-chat.spec.ts`

**Interfaces:**
- Consumes: `silent stop` scenario; `load_session` handler returning a stopped turn (Task 1).

- [ ] **Step 1: Write the specs (discover Stop, session reopen, and slash-menu UI)**

Read `src/lib/components/Composer.svelte` (Stop button, `/clear` slash menu), `src/lib/components/Sidebar.svelte` (reopen a session), and how the 'Stopped' marker renders (search components for "Stopped"). Assertions:
```ts
import { test, expect } from "@playwright/test";

test("R-3: empty-stop 'Stopped' marker shows live and survives reopen", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("silent stop please");
  await composer.press("Enter");
  // Click Stop before any text appears (discover the Stop control).
  // ... click Stop ...
  await expect(page.getByText("Stopped")).toBeVisible();
  // Reopen the session (mock load_session returns the stopped turn) and re-assert.
  // ... reopen via Sidebar ...
  await expect(page.getByText("Stopped")).toBeVisible();
});

test("R-8: /clear is deterministic — clears with content, no-op when empty, never deadlocks", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("some text /clear");        // adjust to trigger the slash menu per the real component
  // invoke /clear with content → composer clears
  // ... discover slash-menu interaction ...
  await expect(composer).toHaveValue("");
  // /clear on empty composer → still responsive (type after it succeeds)
  await composer.fill("");
  // ... invoke /clear on empty ...
  await composer.fill("still works");
  await expect(composer).toHaveValue("still works");
});
```

- [ ] **Step 2: Run — iterate until green**

Run: `cd crates/zanto-desktop && pnpm test:ui -- tests/ui/regression-chat.spec.ts`
Expected: PASS. Adjust selectors/interactions to the real Composer/Sidebar. For R-3 reopen, the Sidebar must list a session to click — the mock `list_sessions`/`list_sessions_page` already returns one (from the seeded fixture). If reopen needs a specific session id, the `load_session` handler ignores the id and returns the stopped turn.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/regression-chat.spec.ts
git commit -m "test(desktop): R-3 stop-marker persistence, R-8 /clear determinism"
```

---

### Task 5: R-6 — finance summary inline

**Files:**
- Create: `crates/zanto-desktop/tests/ui/regression-finance.spec.ts`

**Interfaces:**
- Consumes: `finance` app in `list_apps` (Task 1); `finance summary` scenario; `query_app`/`run_app_action` handlers.

- [ ] **Step 1: Write the spec (discover finance mount + summary rendering)**

Read how an app is mounted/switched (Sidebar app switcher) and `src/lib/apps/finance/monthly_summary.svelte` for the summary node selector. Assertions:
```ts
import { test, expect } from "@playwright/test";

test("R-6: monthly_summary renders inline as a block, no tool-call card", async ({ page }) => {
  await page.goto("/");
  // Switch to the Finance app (discover the app switcher control).
  // ... mount finance ...
  const composer = page.getByRole("textbox").first();
  await composer.fill("finance summary for this month");
  await composer.press("Enter");
  // The monthly_summary component renders inline (discover its selector / a stable label).
  // ... assert summary visible ...
  // No tool-call card for the summary tool.
  await expect(page.getByText("monthly_summary")).toHaveCount(0);
});
```

- [ ] **Step 2: Run — iterate until green**

Run: `cd crates/zanto-desktop && pnpm test:ui -- tests/ui/regression-finance.spec.ts`
Expected: PASS. If `monthly_summary.svelte` expects a different `data` shape than the `summaryBlock.data` in `scenarios.ts`, align it. If mounting finance triggers `query_app`/`run_app_action` calls the mock doesn't satisfy, the Task-1 handlers return canned data; extend only if a specific call throws "no handler".

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/regression-finance.spec.ts crates/zanto-desktop/src/lib/mock/scenarios.ts
git commit -m "test(desktop): R-6 monthly_summary renders inline without tool-call card"
```

---

### Task 6: R-9 — app-switch scoping

**Files:**
- Create: `crates/zanto-desktop/tests/ui/regression-app-switch.spec.ts`

**Interfaces:**
- Consumes: two apps (`chat`, `finance`) in `list_apps`; `mount_app` handler.

- [ ] **Step 1: Write the spec (deterministic post-switch assertion)**

Read the Sidebar app switcher and how the active app is indicated. Prefer asserting the post-switch state (active app changed; not stuck) over the transient 'Switching…' spinner. Assertions:
```ts
import { test, expect } from "@playwright/test";

test("R-9: switching apps activates the target app and doesn't get stuck", async ({ page }) => {
  await page.goto("/");
  // Switch Chat -> Finance, then Finance -> Chat (discover the switcher controls).
  // Assert the active app indicator reflects each switch and the UI remains responsive.
  // If a 'Switching…' indicator is reliably observable without arbitrary waits, assert it appears; otherwise assert the settled post-switch state.
  // ... interactions + assertions discovered from the real UI ...
});
```

- [ ] **Step 2: Run — iterate until green**

Run: `cd crates/zanto-desktop && pnpm test:ui -- tests/ui/regression-app-switch.spec.ts`
Expected: PASS. Keep assertions on settled state to avoid timing flakiness; the mock `mount_app` resolves immediately.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/regression-app-switch.spec.ts
git commit -m "test(desktop): R-9 app-switch activates target app, no stuck state"
```

---

### Task 7: R-4 + R-5 — Rust logic tests

**Files:**
- Modify: `crates/zanto-core/src/config.rs` (test module ~line 646)
- Modify: `crates/zanto-desktop/src-tauri/src/apps/finance/import.rs` (test module) and/or `.../finance/mod.rs`

**Interfaces:**
- Consumes: `Settings` (with `selected_skill: Option<String>`, `save()`/`load()`/`load_file()`) in config.rs; `coerce_amount(Option<&serde_json::Value>) -> f64` in `apps/finance/import.rs`.

- [ ] **Step 1: R-5 — write the failing test for amount coercion (RED)**

In `crates/zanto-desktop/src-tauri/src/apps/finance/import.rs`, inside its `#[cfg(test)] mod tests`, add (read the existing tests there to match style and the exact `coerce_amount` signature):
```rust
#[test]
fn coerce_amount_parses_currency_strings() {
    use serde_json::json;
    assert_eq!(coerce_amount(Some(&json!("$12.50"))), 12.50);
    assert_eq!(coerce_amount(Some(&json!("12.50"))), 12.50);
    assert_eq!(coerce_amount(Some(&json!(12.50))), 12.50);
    assert_eq!(coerce_amount(None), 0.0);
}
```
(If `coerce_amount` is private to the module, the test in the same module can call it directly. Adjust the assertions to the function's real signature/return — discover it.)

- [ ] **Step 2: Run R-5 — expect PASS (the fix already exists)**

Run: `cargo test -p zanto-desktop apps::finance::import` (or the module path the test lives in)
Expected: PASS — this guards the existing 2026-06-18 fix. If it FAILS, that is a real regression finding — report it, do not weaken the test.

- [ ] **Step 3: R-4 — write the Settings skill-persistence round-trip test**

In `crates/zanto-core/src/config.rs` test module, read the existing tests (they show how `Settings` is constructed and how `save`/`load_file` are exercised with a temp path). Add a test that sets `selected_skill = Some("reviewer".into())`, saves to a temp path, loads it back, and asserts `selected_skill` survived. Mirror the existing tests' temp-file/helper pattern exactly. Example skeleton (adapt to the real API):
```rust
#[test]
fn selected_skill_persists_round_trip() {
    let dir = tempfile::tempdir().unwrap();        // or the helper the other tests use
    let path = dir.path().join("settings.json");
    let mut s = Settings::default();               // or however tests build one
    s.selected_skill = Some("reviewer".to_string());
    // save to `path`, then load from `path` (use the same mechanism the existing tests use — save()/load_file())
    let loaded = Settings::load_file(path).expect("load");
    assert_eq!(loaded.selected_skill.as_deref(), Some("reviewer"));
}
```

- [ ] **Step 4: Run R-4 — expect PASS**

Run: `cargo test -p zanto-core config`
Expected: PASS. If `Settings::default()` or the save/load mechanism differs, adapt to the real API shown by the neighboring tests (do not invent helpers).

- [ ] **Step 5: Full test run**

Run: from repo root `cargo test`
Expected: all pass (existing 143 + the 2 new).

- [ ] **Step 6: Commit**

```bash
git add crates/zanto-core/src/config.rs crates/zanto-desktop/src-tauri/src/apps/finance/import.rs
git commit -m "test(core/finance): R-4 skill persists round-trip, R-5 amount coercion"
```

---

### Task 8: CSV automation provenance

**Files:**
- Modify: `docs/zanto-test-checklist.csv`

- [ ] **Step 1: Append an `Automation` column**

Add `Automation` as a new trailing header column. The CSV has multi-line quoted cells — preserve existing quoting/structure exactly; only append one field per row. Populate:
- R-1 → `auto: tests/ui/regression-artifacts.spec.ts`
- R-2 → `auto: tests/ui/regression-artifacts.spec.ts`
- R-3 → `auto: tests/ui/regression-chat.spec.ts`
- R-4 → `auto: core config.rs (selected_skill_persists_round_trip)`
- R-5 → `auto: finance import.rs (coerce_amount_parses_currency_strings)`
- R-6 → `auto: tests/ui/regression-finance.spec.ts`
- R-7 → `auto: tests/ui/regression-artifacts.spec.ts`
- R-8 → `auto: tests/ui/regression-chat.spec.ts`
- R-9 → `auto: tests/ui/regression-app-switch.spec.ts`
- Layer-C rows → `manual-only: <reason>`: S1, S2 (`real app/keychain`); W-1, W-2, W-3, W-4 (`tauri window/OS`); FB-3, FG-4 (`OS notification`); DOC-4 (`live multimodal model`); CO-1 (`live model steering`), CO-3 (`live network`); FLOW-1..FLOW-8 (`end-to-end live model+tools`).
- All other rows → leave the `Automation` field empty.

- [ ] **Step 2: Verify CSV integrity**

Run: `python3 -c "import csv; rows=list(csv.reader(open('docs/zanto-test-checklist.csv'))); print(len(rows), 'rows'); assert all(len(r)==len(rows[0]) for r in rows), 'ragged columns'"`
Expected: prints the row count and no assertion error (every row has the same column count, proving the multi-line quoted cells survived and the new column is consistent).

- [ ] **Step 3: Commit**

```bash
git add docs/zanto-test-checklist.csv
git commit -m "docs: mark R-1..R-9 automated + Layer-C rows manual-only in checklist"
```

---

## Self-Review

**Spec coverage:**
- Scenario router + new handlers + fixtures → Task 1. ✓
- R-1, R-2 → Task 2; R-7 → Task 3; R-3, R-8 → Task 4; R-6 → Task 5; R-9 → Task 6. ✓
- R-4, R-5 (Rust) → Task 7. ✓ (Corrected location: R-5 is `apps/finance/import.rs` in the desktop crate, not `zanto-core/src/data/mod.rs` as the spec drafted — `coerce_amount` lives there.)
- CSV annotation (automated + Layer-C manual-only) → Task 8. ✓
- Contract tests for every new fixture → Task 1 Step 1/3. ✓

**Placeholder scan:** UI spec bodies intentionally carry "discover the selector" guidance with concrete assertions and the exact scenario triggers — this matches the established pattern (selectors are environment facts found by reading real components, not inventable in the plan). All scenario/backend/fixture/Rust code is concrete. No TBD/TODO.

**Type consistency:** `pickScenario`/`Scenario`/`ScenarioEvent`/`defaultScenario` consistent across `scenarios.ts` and `backend.ts`. Triggers (`"chart"`, `"chart with toolcall"`, `"finance summary"`, `"silent stop"`) match between scenarios and the specs that send them. `pin_artifact_cmd` returns a number (i64) consistently in fixture, handler, and contract test. `PinnedArtifact` fields match across the Rust struct, fixtures, and the TS `PinnedArtifact` type.

**Ordering note:** `"chart with toolcall"` scenario is listed before `"chart"` in `scenarios` so the substring match resolves correctly (a message containing "chart with toolcall" also contains "chart"). Verified in Task 1 Step 6.
