# Split-Bridge Testing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Test the zanto-desktop Svelte UI and Rust backend independently across the Tauri IPC seam, with shared golden fixtures that prevent silent drift, all running headless in CI.

**Architecture:** A Vite `--mode mock` alias swaps the four Tauri API modules (`core`, `event`, `webviewWindow`, `plugin-os`) for in-memory mocks that dispatch `invoke` to a typed fake backend seeded from JSON fixtures. Playwright drives the real UI in headless Chromium. The same fixtures are deserialized by a Rust `cargo test` into the real DTOs — the authoritative shape/enum check. Production `tauri dev`/`build` is untouched.

**Tech Stack:** SvelteKit 2 + Svelte 5, Vite 6, Tauri 2 (Rust), `@playwright/test` (new dev dep), `cargo test` + serde.

## Global Constraints

- Desktop client only. CLI / `zanto-core` public API out of scope except existing tests.
- `crates/zanto-desktop/src/lib/ipc.ts` and its ~25 consumers MUST NOT change. The only files allowed to import `@tauri-apps/api/*` or `@tauri-apps/plugin-os` are `ipc.ts` and `src/lib/mock/`.
- Vite alias applies ONLY under `--mode mock`. `tauri dev`/`tauri build` resolve the real packages.
- Mock dev server port `1430` (`strictPort`), distinct from Tauri's `1420`.
- Only one new JS dependency permitted: `@playwright/test`. No vitest, no codegen (`ts-rs`/`typeshare`), no `ts-json-schema-generator`.
- Fixtures authored as JSON (Rust-readable). Rust serde deserialization is the authoritative contract check; TS relies on `svelte-check` (typed mock handlers + real consumers).
- `#[tauri::command]` bodies stay thin — no new business logic in the command layer.
- All commands run from `crates/zanto-desktop/` unless noted. Cargo from repo root.

---

### Task 1: Playwright harness boots against a static page

Proves the headless test runner works before any mock logic exists.

**Files:**
- Modify: `crates/zanto-desktop/package.json` (add dev dep + scripts)
- Create: `crates/zanto-desktop/playwright.config.ts`
- Create: `crates/zanto-desktop/tests/ui/harness.spec.ts`

**Interfaces:**
- Produces: `pnpm test:ui` runs Playwright headless; `playwright.config.ts` exposes `webServer` running `pnpm dev:mock` at `http://localhost:1430`.

- [ ] **Step 1: Add dependency and scripts**

In `package.json`, add to `devDependencies`:
```json
"@playwright/test": "^1.48.0"
```
Add to `scripts`:
```json
"dev:mock": "vite dev --mode mock",
"test:ui": "playwright test"
```

- [ ] **Step 2: Install**

Run: `cd crates/zanto-desktop && pnpm install && pnpm exec playwright install chromium`
Expected: chromium downloaded, no errors.

- [ ] **Step 3: Write Playwright config**

Create `playwright.config.ts`:
```ts
import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/ui",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  reporter: process.env.CI ? "list" : "html",
  use: { baseURL: "http://localhost:1430", trace: "on-first-retry" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "pnpm dev:mock",
    url: "http://localhost:1430",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
```

- [ ] **Step 4: Write the harness smoke test**

Create `tests/ui/harness.spec.ts`:
```ts
import { test, expect } from "@playwright/test";

test("dev:mock server serves the app shell", async ({ page }) => {
  await page.goto("/");
  // The root mounts a full-screen container; assert it exists.
  await expect(page.locator("div.h-screen.w-screen")).toBeVisible();
});
```

- [ ] **Step 5: Run — expect FAIL (mock mode not wired yet)**

Run: `pnpm test:ui`
Expected: FAIL — `vite dev --mode mock` boots but the app throws on the first real `invoke` (no Tauri), so the shell may not render. This failure is expected and motivates Task 2.

- [ ] **Step 6: Commit**

```bash
git add crates/zanto-desktop/package.json crates/zanto-desktop/pnpm-lock.yaml \
  crates/zanto-desktop/playwright.config.ts crates/zanto-desktop/tests/ui/harness.spec.ts
git commit -m "test(desktop): add playwright harness + dev:mock script"
```

---

### Task 2: Mock Tauri modules + vite mock alias + bootable fake backend

The vertical slice that makes the app render in a browser with no Tauri.

**Files:**
- Create: `crates/zanto-desktop/src/lib/mock/event.ts`
- Create: `crates/zanto-desktop/src/lib/mock/webviewWindow.ts`
- Create: `crates/zanto-desktop/src/lib/mock/os.ts`
- Create: `crates/zanto-desktop/src/lib/mock/core.ts`
- Create: `crates/zanto-desktop/src/lib/mock/backend.ts`
- Create: `crates/zanto-desktop/contract/fixtures/{get_config,list_apps,get_catalogue,list_sessions,new_session}.json`
- Modify: `crates/zanto-desktop/vite.config.js`

**Interfaces:**
- Consumes: the `ipc.ts` import surface (must export `invoke`, `listen`, `UnlistenFn`, `getCurrentWebviewWindow`, `platform`).
- Produces:
  - `core.ts` → `invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>`
  - `event.ts` → `listen<T>(event, cb): Promise<UnlistenFn>`, `emit(event: string, payload: unknown): void`, `resetBus(): void`, `type UnlistenFn = () => void`
  - `webviewWindow.ts` → `getCurrentWebviewWindow()`, `emitDrop(payload)`
  - `os.ts` → `platform(): string`, `setPlatform(p: string): void`
  - `backend.ts` → `backend: Record<string, (args: any) => Promise<unknown>>`, `resetBackend(): void`

- [ ] **Step 1: Write the event bus mock**

Create `src/lib/mock/event.ts`:
```ts
// Mock of @tauri-apps/api/event. Aliased in --mode mock. Defines its own types
// so it never re-imports the (aliased) real module.
export type UnlistenFn = () => void;
export type EventCallback<T> = (event: { payload: T }) => void;

type Handler = (payload: unknown) => void;
const handlers = new Map<string, Set<Handler>>();

export async function listen<T>(event: string, cb: EventCallback<T>): Promise<UnlistenFn> {
  const h: Handler = (p) => cb({ payload: p as T });
  let set = handlers.get(event);
  if (!set) handlers.set(event, (set = new Set()));
  set.add(h);
  return () => { set!.delete(h); };
}

// Backend/test-facing: deliver an event payload to all current listeners.
export function emit(event: string, payload: unknown): void {
  handlers.get(event)?.forEach((h) => h(payload));
}

export function resetBus(): void {
  handlers.clear();
}
```

- [ ] **Step 2: Write the webview + os mocks**

Create `src/lib/mock/webviewWindow.ts`:
```ts
type DragPayload = { type: "enter" | "over" | "leave" | "drop"; paths: string[] };
type DropHandler = (e: { payload: DragPayload }) => void;
let dropHandler: DropHandler | null = null;

export function getCurrentWebviewWindow() {
  return {
    onDragDropEvent(cb: DropHandler) {
      dropHandler = cb;
      return Promise.resolve(() => { dropHandler = null; });
    },
  };
}

// Test-facing: simulate a native file drop.
export function emitDrop(payload: DragPayload): void {
  dropHandler?.({ payload });
}
```

Create `src/lib/mock/os.ts`:
```ts
let current = "linux";
export function platform(): string { return current; }
export function setPlatform(p: string): void { current = p; }
```

- [ ] **Step 3: Write the invoke dispatcher**

Create `src/lib/mock/core.ts`:
```ts
import { backend } from "./backend";

// Mock of @tauri-apps/api/core `invoke`. Dispatches by command name to the
// in-memory fake backend. Unknown command names throw so a missing handler
// surfaces loudly in tests instead of silently returning undefined.
export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const handler = backend[cmd];
  if (!handler) throw new Error(`mock invoke: no handler for "${cmd}"`);
  return (await handler(args)) as T;
}
```

- [ ] **Step 4: Write the boot fixtures**

Create `contract/fixtures/get_config.json` (shape must match `ConfigDto`/`Config`):
```json
{
  "request": {},
  "response": {
    "model": "claude-opus-4-8",
    "endpoint": "https://api.anthropic.com",
    "allowed_paths": ["/home/test"],
    "project_dir": null,
    "context_sources": [],
    "selected_skill": null,
    "max_context_turns": null,
    "providers": [
      { "provider": "anthropic", "model": "claude-opus-4-8", "endpoint": null, "has_key": true }
    ],
    "active_provider": "anthropic"
  }
}
```

Create `contract/fixtures/list_apps.json`:
```json
{
  "request": {},
  "response": [
    { "id": "chat", "name": "Chat", "description": "General chat", "stores": [], "components": [], "start_actions": [] }
  ]
}
```

Create `contract/fixtures/get_catalogue.json`:
```json
{ "request": {}, "response": [] }
```

Create `contract/fixtures/list_sessions.json`:
```json
{ "request": {}, "response": [] }
```

Create `contract/fixtures/new_session.json`:
```json
{ "request": {}, "response": "sess-test-0001" }
```

> Note: if the real `AppManifest`/`ConfigDto` JSON shapes differ from the above, the Rust contract test in Task 4 will fail and pin the exact correction — fix the fixture then, not by guessing here.

- [ ] **Step 5: Write the fake backend (boot slice)**

Create `src/lib/mock/backend.ts`. Handlers are annotated with the real `ipc` return types so `svelte-check` validates fixture shapes (non-union fields):
```ts
import type {
  Config, AppManifest, ArtifactDef, SessionMeta, ChatTurn,
} from "$lib/ipc";
import { emit } from "./event";

import getConfigFx from "../../../contract/fixtures/get_config.json";
import listAppsFx from "../../../contract/fixtures/list_apps.json";
import getCatalogueFx from "../../../contract/fixtures/get_catalogue.json";
import listSessionsFx from "../../../contract/fixtures/list_sessions.json";
import newSessionFx from "../../../contract/fixtures/new_session.json";

// Each handler is keyed by the exact `invoke` command name used in ipc.ts.
// Typed return values turn the fixture JSON into a compile-time contract.
export const backend: Record<string, (args: any) => Promise<unknown>> = {
  get_config: async (): Promise<Config> => getConfigFx.response,
  list_apps: async (): Promise<AppManifest[]> => listAppsFx.response,
  get_catalogue: async (): Promise<ArtifactDef[]> => getCatalogueFx.response,
  list_sessions: async (): Promise<SessionMeta[]> => listSessionsFx.response,
  new_session: async (): Promise<string> => newSessionFx.response,
  mount_app: async () => undefined,
  unmount_app: async () => undefined,
  interrupt_turn: async () => undefined,
};

export function resetBackend(): void {
  // re-seed mutable state here as commands with side effects are added.
}

// Silence unused-import lint until streaming handlers (Task 3) use it.
void emit;
void (null as unknown as ChatTurn);
```

> If a handler return type triggers a string-literal widening error from a JSON import (e.g. a discriminated-union field), annotate that single value with `as <Type>` and rely on Task 4's Rust serde test for its exact shape. Prefer no cast where the type checks cleanly.

- [ ] **Step 6: Wire the vite mock alias**

In `vite.config.js`, change the export to receive `mode` and add the alias + port under mock mode:
```js
export default defineConfig(async ({ mode }) => {
  const mock = mode === "mock";
  const mockAlias = mock
    ? {
        "@tauri-apps/api/core": "/src/lib/mock/core.ts",
        "@tauri-apps/api/event": "/src/lib/mock/event.ts",
        "@tauri-apps/api/webviewWindow": "/src/lib/mock/webviewWindow.ts",
        "@tauri-apps/plugin-os": "/src/lib/mock/os.ts",
      }
    : {};
  return {
    plugins: [tailwindcss(), sveltekit()],
    resolve: { alias: mockAlias },
    clearScreen: false,
    server: {
      port: mock ? 1430 : 1420,
      strictPort: true,
      host: host || false,
      hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
      watch: { ignored: ["**/src-tauri/**"] },
    },
  };
});
```

- [ ] **Step 7: Run the boot smoke**

Run: `pnpm test:ui -- tests/ui/harness.spec.ts`
Expected: PASS — app shell renders. If an `invoke` throws "no handler for X", add `X` to `backend` (boot calls only need the handlers above; `initStreaming` uses `listen`, already mocked).

- [ ] **Step 8: Verify production build path untouched**

Run: `pnpm check`
Expected: PASS — no type errors; mock files type-check against `ipc.ts`.

- [ ] **Step 9: Commit**

```bash
git add crates/zanto-desktop/src/lib/mock crates/zanto-desktop/contract/fixtures \
  crates/zanto-desktop/vite.config.js
git commit -m "test(desktop): mock tauri bridge + vite mock mode (bootable UI)"
```

---

### Task 3: Streaming send_message handler + chat flow spec

Exercises the event seam end to end in the browser.

**Files:**
- Create: `crates/zanto-desktop/contract/fixtures/send_message.json`
- Modify: `crates/zanto-desktop/src/lib/mock/backend.ts`
- Create: `crates/zanto-desktop/tests/ui/chat.spec.ts`

**Interfaces:**
- Consumes: `emit` from `event.ts`; `send_message`/`interrupt_turn` command names; chat events `chat_chunk`, `chat_done`, `chat_stopped`.
- Produces: `send_message` handler that emits a scripted stream then resolves a `ChatTurn`.

- [ ] **Step 1: Write the streaming fixture**

Create `contract/fixtures/send_message.json`:
```json
{
  "request": { "text": "hello", "imagePaths": [] },
  "response": { "blocks": [{ "kind": "markdown", "text": "Hi there." }] },
  "events": [
    { "event": "chat_chunk", "payload": { "text": "Hi " } },
    { "event": "chat_chunk", "payload": { "text": "there." } },
    { "event": "chat_done", "payload": null }
  ]
}
```

- [ ] **Step 2: Implement the streaming handler**

In `backend.ts`, add the import and handler, and remove the `void emit;`/`void (... ChatTurn)` placeholders:
```ts
import sendMessageFx from "../../../contract/fixtures/send_message.json";

let interrupted = false;

backend.send_message = async (): Promise<ChatTurn> => {
  interrupted = false;
  for (const ev of sendMessageFx.events) {
    if (interrupted) break;
    emit(ev.event, ev.payload);
    await Promise.resolve(); // yield a microtask so the UI updates between deltas
  }
  if (interrupted) { emit("chat_stopped", null); emit("chat_done", null); }
  return sendMessageFx.response as ChatTurn;
};

backend.interrupt_turn = async () => { interrupted = true; };
```
(Keep the `ChatTurn` import added in Task 2; delete the two `void` placeholder lines.)

- [ ] **Step 3: Write the chat flow spec**

Create `tests/ui/chat.spec.ts`:
```ts
import { test, expect } from "@playwright/test";

test("sending a message renders a streamed assistant reply", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("hello");
  await composer.press("Enter");
  await expect(page.getByText("Hi there.")).toBeVisible();
});
```

- [ ] **Step 4: Run**

Run: `pnpm test:ui -- tests/ui/chat.spec.ts`
Expected: PASS. If the composer selector or send key differs, inspect `src/lib/components/Composer.svelte` and adjust the locator (do not change app code).

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src/lib/mock/backend.ts \
  crates/zanto-desktop/contract/fixtures/send_message.json \
  crates/zanto-desktop/tests/ui/chat.spec.ts
git commit -m "test(desktop): streaming send_message mock + chat flow spec"
```

---

### Task 4: Rust contract test over fixtures

Authoritative shape/enum check: every fixture must deserialize into the real DTOs.

**Files:**
- Create: `crates/zanto-desktop/src-tauri/tests/contract.rs`
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/mod.rs` (add `Deserialize` to response DTOs)
- Modify: `crates/zanto-desktop/src-tauri/Cargo.toml` (ensure `serde_json` is a dev-dep if not already a dep)

**Interfaces:**
- Consumes: `contract/fixtures/*.json`; the public DTO types (`zanto_desktop_lib::ipc::ConfigDto`, etc.). Confirm the lib crate name in `Cargo.toml` (`[lib] name`); the test references it as `use <libname>::...`.
- Produces: a `cargo test` target `contract` asserting each fixture's `response` deserializes into its DTO.

- [ ] **Step 1: Confirm the lib crate name and DTO visibility**

Run: `grep -nE "^name|^\[lib\]|crate-type" crates/zanto-desktop/src-tauri/Cargo.toml`
Note the `[lib] name` (e.g. `zanto_desktop_lib`). The integration test imports DTOs from it. Ensure `pub mod ipc;` and the DTO structs are `pub` (they are per the spec exploration).

- [ ] **Step 2: Add `Deserialize` to response DTOs**

In `src-tauri/src/ipc/mod.rs`, change the derives on response DTOs so the test can deserialize fixtures. For each of `RenderMsg`, `ProviderDto`, `ConfigDto`:
```rust
#[derive(Serialize, Deserialize)]
```
Ensure `use serde::{Deserialize, Serialize};` is present at the top of the file.

- [ ] **Step 3: Write the failing contract test**

Create `src-tauri/tests/contract.rs`. Map each fixtured command to its response DTO. Start with the desktop-local `get_config` (others added as their DTOs gain `Deserialize`):
```rust
//! Contract test: every fixture's `response` must deserialize into the real DTO.
use std::fs;
use std::path::Path;

// Adjust the crate path to the actual [lib] name from Cargo.toml.
use zanto_desktop_lib::ipc::ConfigDto;

fn fixture(name: &str) -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contract/fixtures")
        .join(format!("{name}.json"));
    let raw = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&raw).expect("fixture is valid JSON")
}

#[test]
fn get_config_response_matches_dto() {
    let fx = fixture("get_config");
    let _dto: ConfigDto = serde_json::from_value(fx["response"].clone())
        .expect("get_config response deserializes into ConfigDto");
}
```

- [ ] **Step 4: Run — verify it fails first if a derive is missing, then passes**

Run: `cargo test -p zanto-desktop --test contract` (use the actual package name from `Cargo.toml`; find via `grep '^name' crates/zanto-desktop/src-tauri/Cargo.toml`)
Expected: PASS once `Deserialize` is on `ConfigDto`/`ProviderDto`. If the fixture shape is wrong, the panic message names the offending field — fix `contract/fixtures/get_config.json` to match the DTO.

- [ ] **Step 5: Extend to remaining boot fixtures (iteration)**

For each of `list_apps`, `get_catalogue`, `list_sessions`, `send_message`: identify the response DTO (search `src-tauri/src/ipc/` and `zanto-core` for the return type), add `Deserialize` where the compiler demands, add a `#[test]` mirroring Step 3. Run `cargo test ... --test contract` after each; commit per command or in one batch.

- [ ] **Step 6: Commit**

```bash
git add crates/zanto-desktop/src-tauri/tests/contract.rs \
  crates/zanto-desktop/src-tauri/src/ipc/mod.rs
git commit -m "test(desktop): rust contract test deserializes fixtures into DTOs"
```

---

### Task 5: Seam guard

Fails if `@tauri-apps/api` is imported outside the seam.

**Files:**
- Create: `crates/zanto-desktop/tests/ui/seam.spec.ts`

**Interfaces:**
- Consumes: filesystem under `src/`. Runs as a Playwright test (node context) so it shares the existing runner — no new dep.

- [ ] **Step 1: Write the guard test**

Create `tests/ui/seam.spec.ts`:
```ts
import { test, expect } from "@playwright/test";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const ROOT = new URL("../../src", import.meta.url).pathname;
const ALLOWED = ["lib/ipc.ts", "lib/mock/"]; // relative to src/

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? walk(p) : [p];
  });
}

test("tauri api is imported only from the ipc seam and mocks", () => {
  const offenders: string[] = [];
  for (const file of walk(ROOT)) {
    if (!/\.(ts|svelte)$/.test(file)) continue;
    const rel = file.slice(ROOT.length + 1);
    if (ALLOWED.some((a) => rel === a || rel.startsWith(a))) continue;
    const src = readFileSync(file, "utf8");
    if (/from\s+["']@tauri-apps\/(api|plugin-os)/.test(src)) offenders.push(rel);
  }
  expect(offenders, `stray @tauri-apps imports: ${offenders.join(", ")}`).toEqual([]);
});
```

- [ ] **Step 2: Run**

Run: `pnpm test:ui -- tests/ui/seam.spec.ts`
Expected: PASS (only `ipc.ts` and `mock/` import Tauri).

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/seam.spec.ts
git commit -m "test(desktop): seam guard forbids stray tauri imports"
```

---

### Task 6: CI workflow

Two headless jobs: rust + web.

**Files:**
- Create: `.github/workflows/test.yml`

- [ ] **Step 1: Write the workflow**

Create `.github/workflows/test.yml`:
```yaml
name: test
on: [push, pull_request]
jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test
  web:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: crates/zanto-desktop
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with: { version: 9.15.9 }
      - uses: actions/setup-node@v4
        with: { node-version: 24, cache: pnpm, cache-dependency-path: crates/zanto-desktop/pnpm-lock.yaml }
      - run: pnpm install --frozen-lockfile
      - run: pnpm check
      - run: pnpm exec playwright install --with-deps chromium
      - run: pnpm test:ui
```

> Note: `cargo test` needs the Tauri build deps on Linux. If the `rust` job fails to compile `zanto-desktop`'s lib, add a step installing them (`libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`) before `cargo test`, or scope the job to `cargo test -p zanto-core` plus the contract test target only.

- [ ] **Step 2: Validate locally**

Run: `cd /home/lazy/dev/github/local-work && cargo test && cd crates/zanto-desktop && pnpm check && pnpm test:ui`
Expected: all PASS. (CI run itself verified on push.)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/test.yml
git commit -m "ci: headless rust + web test jobs"
```

---

### Task 7: Document the seam, guardrail, and fixture loop

**Files:**
- Modify: `trd.md`

- [ ] **Step 1: Add a "Testing" section to `trd.md`**

Document, in prose matching the file's style:
- The single Tauri seam (`ipc.ts`) and the `--mode mock` alias.
- The thin-command guardrail: business logic lives in `zanto-core`; commands only adapt + emit.
- How to add a command: add a `#[tauri::command]`, register it in `lib.rs`, add a typed handler in `src/lib/mock/backend.ts`, add `contract/fixtures/<command>.json`, add a `#[test]` in `contract.rs` (adding `Deserialize` to the response DTO if missing).
- Commands: `pnpm dev:mock`, `pnpm test:ui`, `cargo test`.

- [ ] **Step 2: Commit**

```bash
git add trd.md
git commit -m "docs: document split-bridge testing seam + fixture workflow"
```

---

## Self-Review

**Spec coverage:**
- UI track (vite alias, mocks, fake backend, Playwright) → Tasks 1–3. ✓
- Backend track (thin commands, core tests) → guardrail in Task 4/7; core tests are existing `cargo test`. ✓
- Contract (golden fixtures, Rust serde, TS svelte-check) → Tasks 2 (fixtures+typed handlers) & 4 (Rust). ✓
- Automation (scripts, CI) → Tasks 1 & 6. ✓
- Seam guard → Task 5. ✓
- Docs → Task 7. ✓

**Known follow-ups (intentionally iterative, not gaps):** fixtures + contract tests start with the boot/chat slice; remaining commands are added one fixture at a time per the Task 4 Step 5 / Task 7 loop. This matches the "focus on iterations" goal.

**Type consistency:** `backend` record keyed by exact `invoke` command names from `ipc.ts`; `emit`/`listen`/`UnlistenFn` consistent across `event.ts`, `core.ts`, `backend.ts`; `ConfigDto` derive change matches the `contract.rs` import.

**Open verification points flagged inline (not placeholders):** exact `[lib]` crate name (Task 4 Step 1), real `AppManifest`/`ConfigDto` JSON shape (Task 2 Step 4 note + Task 4 Step 4), composer selector (Task 3 Step 4), Linux Tauri build deps in CI (Task 6 note). Each has a concrete discovery command and correction path.
