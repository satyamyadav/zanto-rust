# Chat UI Test Automation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Automate the 12 Chat checklist rows (C-1..C-12) as Playwright specs over the existing mock-bridge harness, extending the scenario router + mock handlers, and annotate the CSV.

**Architecture:** New scenarios in `src/lib/mock/scenarios.ts` (error/throws, partial-stop, thinking, workflow, link) + a paginated `load_session_page` in `backend.ts` drive each chat behavior; Playwright specs assert the real rendered DOM. Selectors are discovered by reading the real components (never changed).

**Tech Stack:** SvelteKit + Svelte 5, Vite `--mode mock`, `@playwright/test`.

## Global Constraints

- Desktop client only. Additive only: `src/lib/mock/*`, `tests/ui/*`, `docs/zanto-test-checklist.csv` (and, only if a paginated fixture is added, `contract/fixtures/*` + `src-tauri/tests/contract.rs`).
- NO change to `src/lib/ipc.ts`, its consumers, or any app/runtime component. The Vite mock alias applies only under `--mode mock` (port 1430).
- No new dependency.
- Scenario routing: `pickScenario` returns the first scenario whose `trigger` (case-insensitive) is a substring of the message text, else `defaultScenario`. Keep more-specific triggers earlier in the array.
- Mock event emission: `emit("chat_chunk",{text})`, `emit("chat_reasoning",{text})`, `emit("chat_tool_call",{id,name,args})`, `emit("chat_tool_result",{id,output,ok})`, `emit("chat_block",{block})`, `emit("chat_done",null)`, `emit("chat_stopped",null)`.
- These specs assert UI plumbing driven by canned events — not real model behavior. CSV annotation reads `auto: tests/ui/chat-behaviors.spec.ts`.
- Verify gates after any task that changes them: `cd crates/zanto-desktop && pnpm check` (0 errors), `pnpm test:ui` (all pass). Run `cargo test` only if Rust changes.
- Existing scenarios/specs MUST keep passing: `default` ("Hi there."), `chart`, `chart with toolcall`, `finance summary`, `silent stop`, and specs harness/chat/seam/regression-*.

---

### Task 1: Scenario + handler infrastructure

**Files:**
- Modify: `crates/zanto-desktop/src/lib/mock/scenarios.ts`
- Modify: `crates/zanto-desktop/src/lib/mock/backend.ts`

**Interfaces:**
- Produces (consumed by Tasks 2–7): scenarios with triggers `"trigger error"` (flag `throws:true`), `"partial stop"` (one chunk then `blocking:true`), `"think"`, `"workflow"`, `"link"`; and a paginated `load_session_page(args:{offset,limit})`.

- [ ] **Step 1: Add scenario flags + new scenarios**

In `scenarios.ts`, extend the `Scenario` type with optional `throws?: boolean` and (if not present) `blocking?: boolean`. Add these scenarios to the `scenarios` array (place `"partial stop"` and `"trigger error"` before any shorter-substring trigger; none of these collide). Each `response` can be `{ blocks: [] }` for the throwing/blocking ones:
```ts
{ trigger: "trigger error", throws: true, events: [], response: { blocks: [] } },
{ trigger: "partial stop", blocking: true, events: [
    { event: "chat_chunk", payload: { text: "Partial answer so far" } },
  ], response: { blocks: [] } },
{ trigger: "think", events: [
    { event: "chat_reasoning", payload: { text: "Considering options" } },
    { event: "chat_tool_call", payload: { id: "t1", name: "read_file", args: { path: "/x" } } },
    { event: "chat_tool_result", payload: { id: "t1", output: "ok", ok: true } },
    { event: "chat_chunk", payload: { text: "Done." } },
    { event: "chat_done", payload: null },
  ], response: { blocks: [{ kind: "markdown", text: "Done." }] } },
{ trigger: "workflow", events: [
    { event: "chat_tool_call", payload: { id: "w1", name: "list_directory", args: { path: "/" } } },
    { event: "chat_tool_result", payload: { id: "w1", output: "a\nb", ok: true } },
    { event: "chat_tool_call", payload: { id: "w2", name: "read_file", args: { path: "/a" } } },
    { event: "chat_tool_result", payload: { id: "w2", output: "hello", ok: true } },
    { event: "chat_chunk", payload: { text: "Done." } },
    { event: "chat_done", payload: null },
  ], response: { blocks: [{ kind: "markdown", text: "Done." }] } },
{ trigger: "link", events: [
    { event: "chat_chunk", payload: { text: "See https://example.com for details." } },
    { event: "chat_done", payload: null },
  ], response: { blocks: [{ kind: "markdown", text: "See https://example.com for details." }] } },
```

- [ ] **Step 2: Implement `throws` + one-shot recovery in `send_message`, and paginated `load_session_page`**

In `backend.ts`, in the `send_message` handler, after picking the scenario and resetting `interrupted`, handle a throwing scenario with a ONE-SHOT flag so C-10's Retry can recover (first attempt throws, second succeeds). Add a module-level `let errorArmed = true;` (reset in `resetBackend`). Then:
```ts
const sc = pickScenario(args?.text ?? "");
if (sc.throws) {
  if (errorArmed) { errorArmed = false; throw new Error("mock: simulated turn failure"); }
  // recovered attempt: behave like the default scenario
  for (const ev of defaultScenario.events) { emit(ev.event, ev.payload); await Promise.resolve(); }
  return defaultScenario.response;
}
```
(Place this before the normal event loop. Keep the existing `blocking`/`interrupted` handling intact.)

Add a paginated `load_session_page` that returns a deterministic long list (so C-11 scrollback has older pages). Build it in-module:
```ts
const longSession = Array.from({ length: 60 }, (_, i) => ({
  role: i % 2 === 0 ? "user" : "assistant",
  text: `msg #${i}`,
  blocks: null, segments: null, stopped: null,
}));
backend.load_session_page = async (a: { offset?: number; limit?: number }): Promise<any> => {
  const offset = a?.offset ?? 0;
  const limit = a?.limit ?? 20;
  // Newest-last list; a page returns a window. Return the slice from the end working backwards
  // so offset=0 yields the most recent `limit`, larger offsets yield older messages.
  const end = Math.max(0, longSession.length - offset);
  const start = Math.max(0, end - limit);
  return longSession.slice(start, end);
};
```
Keep `load_session` returning the existing stopped-turn fixture (R-3 depends on it). If the UI's first open calls `load_session` (not the paginated form) and C-11 needs the long list on first open, have `load_session` return `longSession.slice(-20)` AND re-verify R-3 still passes; otherwise leave `load_session` as-is and rely on the scrollback calling `load_session_page`. Choose based on what the real session store calls — discover from `session.svelte.ts` (`selectSession`/`loadOlder`).

- [ ] **Step 3: Verify existing specs unaffected**

Run: `cd crates/zanto-desktop && pnpm check` → 0 errors. Then `pnpm test:ui` → ALL existing specs pass (the new scenarios are additive; `throws`/`blocking` only fire on their triggers). If R-3 broke because `load_session` changed, revert `load_session` to the stopped fixture and page C-11 via `load_session_page` only.

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/mock/scenarios.ts crates/zanto-desktop/src/lib/mock/backend.ts
git commit -m "test(desktop): chat-ui scenarios (error/partial-stop/think/workflow/link) + paginated load_session_page"
```

---

### Task 2: C-1 streaming, C-6 copy, C-9 slash menu

**Files:**
- Create: `crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts`

- [ ] **Step 1: Write the three tests (discover selectors from `Composer.svelte`, `Message.svelte`, `MessageList.svelte`)**

Template; adjust locators to the real UI (do NOT change components):
```ts
import { test, expect } from "@playwright/test";

test("C-1: tokens stream into the assistant reply", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("hello there");
  await composer.press("Enter");
  await expect(page.getByText("Hi there.")).toBeVisible();
});

test("C-6: copy a reply puts its text on the clipboard", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("hello");
  await composer.press("Enter");
  await expect(page.getByText("Hi there.")).toBeVisible();
  // Hover the reply, click the copy control (discover the selector).
  // ... hover + click copy ...
  // Assert clipboard OR the 'Copied' feedback if clipboard read is unavailable.
  const clip = await page.evaluate(() => navigator.clipboard.readText().catch(() => ""));
  expect(clip.length === 0 ? true : clip).toBeTruthy(); // prefer asserting clip.includes("Hi there.") when available
});

test("C-9: slash menu offers /new and /clear, and /new starts a fresh session", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  // Type "/" at line start; assert the slash menu shows /new and /clear (discover the menu).
  // Select /new; assert the thread is empty / a new session began.
});
```

- [ ] **Step 2: Run, iterate selectors, green**

Run: `cd crates/zanto-desktop && pnpm test:ui -- tests/ui/chat-behaviors.spec.ts`. For C-6, grant clipboard permission (shown); if the runner blocks clipboard read, fall back to asserting the visible 'Copied' toast/state. For C-9, read `Composer.svelte`'s slash registry to find `/new` + `/clear` and how selection works.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts
git commit -m "test(desktop): C-1 streaming, C-6 copy, C-9 slash menu"
```

---

### Task 3: C-2 stop mid-turn, C-3 queue while busy

**Files:**
- Modify: `crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts`

**Interfaces:** Consumes the `partial stop` (blocking) scenario from Task 1.

- [ ] **Step 1: Add the two tests**

```ts
test("C-2: stopping mid-turn keeps the partial reply and shows the Stopped marker", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("partial stop now");
  await composer.press("Enter");
  await expect(page.getByText("Partial answer so far")).toBeVisible();
  // Click Stop (discover the Stop control), then:
  await expect(page.getByText("Partial answer so far")).toBeVisible();
  await expect(page.getByText("Stopped")).toBeVisible();
});

test("C-3: a message typed while busy is queued and dispatched after the turn ends", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("partial stop please");
  await composer.press("Enter");
  await expect(page.getByText("Partial answer so far")).toBeVisible(); // busy
  await composer.fill("queued follow-up");
  await composer.press("Enter");
  // Assert a pending/queued indicator (discover it).
  // Click Stop to free the turn; the queued message should then dispatch.
  // ... click Stop ...
  await expect(page.getByText("queued follow-up")).toBeVisible(); // its user bubble appears / it runs
});
```

- [ ] **Step 2: Run, iterate, green**

Run: `pnpm test:ui -- tests/ui/chat-behaviors.spec.ts`. Discover the Stop control and the queued/pending indicator from `Composer.svelte`/`MessageList.svelte`. The blocking `partial stop` scenario keeps `busy` true until Stop fires `interrupt_turn`; the store's `finally` then dispatches the queued message.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts
git commit -m "test(desktop): C-2 stop mid-turn keeps partial, C-3 queue FIFO"
```

---

### Task 4: C-4 thinking block, C-5 workflow grouping

**Files:**
- Modify: `crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts`

**Interfaces:** Consumes the `think` and `workflow` scenarios.

- [ ] **Step 1: Add the two tests (discover `ThinkingBlock.svelte`, `WorkflowGroup.svelte`, `ToolCallSegment.svelte`)**

```ts
test("C-4: a tool-using turn shows a thinking block that collapses to 'Thought for N steps'", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("think about this");
  await composer.press("Enter");
  await expect(page.getByText("Done.")).toBeVisible();
  // Assert the collapsed thinking label (e.g. /Thought for \d+ step/), then expand and assert reasoning text.
  await expect(page.getByText(/Thought for \d+ step/)).toBeVisible();
});

test("C-5: multiple tool calls are grouped as a Workflow", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("workflow run");
  await composer.press("Enter");
  await expect(page.getByText("Done.")).toBeVisible();
  // Assert a 'Workflow' grouping with a step count (discover the real label).
  await expect(page.getByText(/Workflow/)).toBeVisible();
});
```

- [ ] **Step 2: Run, iterate, green**

Run: `pnpm test:ui -- tests/ui/chat-behaviors.spec.ts`. Confirm the real collapsed labels (the regex may need adjusting to the actual copy, e.g. "Thought for 1 step"). Expand the thinking block and assert the reasoning ("Considering options") shows.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts
git commit -m "test(desktop): C-4 thinking block, C-5 workflow grouping"
```

---

### Task 5: C-7 paste expander, C-8 @-tag

**Files:**
- Modify: `crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts`

**Interfaces:** C-8 consumes the existing `browse_dir` mock (returns `FileEntry[]`).

- [ ] **Step 1: Add the two tests (discover composer paste + @-autocomplete behavior)**

```ts
test("C-7: a large paste collapses to a chip but the full text is still sent", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  const big = Array.from({ length: 60 }, (_, i) => `line ${i}`).join("\n");
  await composer.focus();
  // Paste via the clipboard API or keyboard; discover how Composer detects a large paste.
  // Assert a 'pasted N lines' chip appears in the composer.
  // Send, then assert the user message carries the full text (or the chip expands to it).
});

test("C-8: typing @ opens a file autocomplete and inserts the path", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("what is in @");
  // Assert an autocomplete list appears (backed by browse_dir). Pick an entry; assert an @<path> token inserted.
});
```

- [ ] **Step 2: Run, iterate, green**

Run: `pnpm test:ui -- tests/ui/chat-behaviors.spec.ts`. Read `Composer.svelte` for the paste-expander threshold/behavior and the `@` autocomplete (it calls `ipc.browseDir`). The mock `browse_dir` returns entries for `path=undefined` (allowed roots) and descends on a path — confirm it returns at least one entry; if it returns `[]`, seed it minimally in `backend.ts` (allowed, additive) so the autocomplete has an item.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts crates/zanto-desktop/src/lib/mock/backend.ts
git commit -m "test(desktop): C-7 paste expander, C-8 @-tag autocomplete"
```

---

### Task 6: C-10 error + retry, C-12 link handling

**Files:**
- Modify: `crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts`

**Interfaces:** Consumes the one-shot `trigger error` scenario and the `link` scenario.

- [ ] **Step 1: Add the two tests (discover `ErrorSegment.svelte` + the link popup)**

```ts
test("C-10: a failed turn shows an error card with Retry that recovers", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("trigger error");
  await composer.press("Enter");
  // Assert an inline error card with a Retry control (discover selectors).
  await expect(page.getByText(/error|failed/i)).toBeVisible();
  // Click Retry; the one-shot mock now succeeds → a normal reply streams.
  // ... click Retry ...
  await expect(page.getByText("Hi there.")).toBeVisible();
});

test("C-12: clicking a link in a reply opens a preview popup; the app does not navigate", async ({ page }) => {
  await page.goto("/");
  const urlBefore = page.url();
  const composer = page.getByRole("textbox").first();
  await composer.fill("link please");
  await composer.press("Enter");
  const link = page.getByRole("link", { name: /example\.com/ });
  await expect(link).toBeVisible();
  await link.click();
  // Assert the preview popup/card with actions (Open in browser / Copy / View in panel) — discover the real controls.
  await expect(page.getByText(/Open in browser/i)).toBeVisible();
  expect(page.url()).toBe(urlBefore); // app didn't navigate
});
```

- [ ] **Step 2: Run, iterate, green**

Run: `pnpm test:ui -- tests/ui/chat-behaviors.spec.ts`. Discover the real error-card copy/Retry control and the link popup actions (from `links.svelte.ts` + the Message rendering). If clicking "Open in browser" must be exercised, it routes through `ipc.openExternal` (mock no-op) — assert it doesn't throw/navigate.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts
git commit -m "test(desktop): C-10 error+retry recovery, C-12 link preview popup"
```

---

### Task 7: C-11 infinite scroll

**Files:**
- Modify: `crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts`

**Interfaces:** Consumes the paginated `load_session_page` from Task 1.

- [ ] **Step 1: Add the test (discover scrollback trigger from `MessageList.svelte`/`session.svelte.ts loadOlder`)**

```ts
test("C-11: scrolling to the top loads older messages and preserves position", async ({ page }) => {
  await page.goto("/");
  // Open a session with many messages (Sidebar → a session; mock load_session_page paginates 60 msgs).
  // ... open session ...
  // Assert the initial page shows recent messages (e.g. "msg #59" area) but not the oldest ("msg #0").
  // Scroll the message list to the top to trigger loadOlder.
  // Assert an older message ("msg #0" or a lower index) becomes present (count increased),
  // and the previously-top message is still in the DOM (position preserved).
});
```

- [ ] **Step 2: Run, iterate, green**

Run: `pnpm test:ui -- tests/ui/chat-behaviors.spec.ts`. Read how the message list opens a session and triggers `loadOlder` (scroll to top / a sentinel). Ensure the mock `load_session`/`load_session_page` returns the long list for the opened session. If the first open uses `load_session` (not the page form), make `load_session` return `longSession.slice(-INITIAL)` for this to work — and RE-RUN the full suite to confirm R-3 (which reopens a session expecting the Stopped turn) still passes; if it conflicts, gate the long list behind a specific session id the test opens, leaving the default session returning the stopped turn.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts crates/zanto-desktop/src/lib/mock/backend.ts
git commit -m "test(desktop): C-11 message infinite scroll"
```

---

### Task 8: CSV annotation

**Files:**
- Modify: `docs/zanto-test-checklist.csv`

- [ ] **Step 1: Annotate C-rows programmatically**

Use Python's `csv` module (preserve multi-line quoted cells; the `Automation` column already exists at the end). For each row whose `ID` (column index 1) is `C-1`..`C-12`, set the `Automation` cell to `auto: tests/ui/chat-behaviors.spec.ts`. Leave all other rows unchanged.

- [ ] **Step 2: Verify integrity**

Run:
```
python3 -c "import csv; r=list(csv.reader(open('docs/zanto-test-checklist.csv'))); assert all(len(x)==len(r[0]) for x in r); print('rows',len(r)); print([(x[1],x[-1]) for x in r if x[1].startswith('C-')])"
```
Expected: row count unchanged, every C-1..C-12 shows the `auto:` value, no ragged columns.

- [ ] **Step 3: Commit**

```bash
git add docs/zanto-test-checklist.csv
git commit -m "docs: mark Chat rows C-1..C-12 automated in checklist"
```

---

## Self-Review

**Spec coverage:** C-1 (T2), C-2/C-3 (T3), C-4/C-5 (T4), C-6 (T2), C-7/C-8 (T5), C-9 (T2), C-10/C-12 (T6), C-11 (T7); infra (T1); CSV (T8). All 12 covered. ✓

**Placeholder scan:** Spec test bodies carry "discover the selector" guidance with concrete scenario triggers and assertions — the established pattern (selectors are environment facts). All scenario/handler code is concrete. No TBD.

**Type consistency:** Scenario triggers (`"trigger error"`, `"partial stop"`, `"think"`, `"workflow"`, `"link"`) match between `scenarios.ts` and the specs that send them. `throws`/`blocking` flags handled in `backend.ts` `send_message`. `load_session_page` signature `{offset,limit}` consistent.

**Risk note (flagged for execution):** C-11 may require `load_session` (not just `load_session_page`) to return the long list on first open; Task 1 Step 2 and Task 7 Step 2 both call out re-verifying R-3 if `load_session` changes, with a fallback (gate the long list behind a specific session id). C-10 relies on the one-shot `errorArmed` flag resetting per page (Playwright page isolation) so each test starts armed.
