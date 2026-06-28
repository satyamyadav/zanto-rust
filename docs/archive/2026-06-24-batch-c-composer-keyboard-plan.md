# Batch C — Composer & keyboard UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use `- [ ]`.

**Goal:** #4 select skills from the composer via a `/skill` command (autocomplete + keyboard); #2 make the `@`-file picker fully keyboard-navigable with path-segment autocomplete; #12 Enter-to-advance/submit in the HITL ask form.

**Architecture:** Frontend-only (`crates/zanto-desktop/src/`). New mock handlers (`list_skills`, `set_active_skill`) + a HITL-form mock scenario so the Playwright harness can drive #4 and #12. Reuse the composer's existing menu state machine and HitlForm focus-trap.

**Tech Stack:** Svelte 5, `@playwright/test`, `pnpm check`.

## Global Constraints

- Frontend only; no Rust/IPC-surface change. No new dependency. `ipc.ts` unchanged (the mock implements existing commands).
- Verify after each task: `cd crates/zanto-desktop && pnpm check` (0 errors) + `pnpm test:ui` (all pass). Discover exact component internals by reading them (line numbers are guides).
- Composer menu state today: `type Menu = "none"|"file"|"slash"`, `SLASH_COMMANDS` (`/new`,`/clear`), `menu`/`query`/`active`/`dirStack` state, `filteredEntries`/`filteredCommands`, `selectEntry(e)` (descends dirs by pushing `dirStack`; inserts a tag for files), arrow/Enter/Esc handlers. `@`-picker calls `ipc.browseDir(path?)`.
- HitlForm today: focus-trap + Esc-to-close; multi-step via `stepIdx` with Back/Next/Submit buttons; fields text/select/confirm.

---

### Task 1: Mock infra — skills + HITL-form scenario

**Files:**
- Modify: `crates/zanto-desktop/src/lib/mock/backend.ts`
- Modify: `crates/zanto-desktop/src/lib/mock/scenarios.ts`

**Interfaces:**
- Produces: mock `list_skills` (returns seeded `SkillDto[]`), `set_active_skill` (records the selected name in mutable state, resettable), and a `hitl form` scenario whose `send_message` emits an `interaction_request` (kind `"form"`) so `HitlForm` renders; the existing `respond` path resolves it.

- [ ] **Step 1: Add skills handlers to `backend.ts`**

```ts
let activeSkill: string | null = null;
backend.list_skills = async (): Promise<{ name: string; preview: string }[]> => [
  { name: "reviewer", preview: "Review code for bugs and clarity." },
  { name: "researcher", preview: "Find and cite sources." },
];
backend.set_active_skill = async (a: { name: string | null }): Promise<void> => { activeSkill = a?.name ?? null; };
```
(Reset `activeSkill = null` wherever other mutable mock state resets, if such a reset exists; otherwise module reload per page handles it.)

- [ ] **Step 2: Add a `hitl form` scenario (for #12)**

In `scenarios.ts`, add a scenario triggered by `"hitl form"` that, instead of normal streaming, emits an `interaction_request` form event then resolves. The HitlForm listens via `onInteractionRequest` (`listen("interaction_request")`). Emit a 2-step form:
```ts
{ trigger: "hitl form", events: [
    { event: "interaction_request", payload: {
        id: "req-1", kind: "form", title: "Tell me about your project",
        steps: [
          { fields: [{ name: "name", label: "Project name", type: "text" }] },
          { fields: [{ name: "lang", label: "Language", type: "select", options: ["rust","ts"] }] },
        ] } },
  ], response: { blocks: [] } },
```
Ensure the mock `respond` handler exists (records the response + resolves); add it to `backend.ts` if missing:
```ts
backend.respond = async (_a: { requestId: string; value: unknown }): Promise<void> => {};
```

- [ ] **Step 3: Verify existing specs unaffected**

Run: `cd crates/zanto-desktop && pnpm check && pnpm test:ui`. Expected: 0 errors; all existing specs still pass (new handlers/scenario are additive; only fire on their triggers/commands).

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/mock/backend.ts crates/zanto-desktop/src/lib/mock/scenarios.ts
git commit -m "test(desktop): mock list_skills/set_active_skill + hitl-form scenario for Batch C"
```

---

### Task 2: #4 — `/skill` command in the composer

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/Composer.svelte`
- Test: `crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts`

**Interfaces:** Consumes mock `list_skills`/`set_active_skill` (Task 1). Reuses the menu state machine.

- [ ] **Step 1: Add a `skill` menu mode + `/skill` command**

Extend `type Menu` to include `"skill"`. Add a `/skill` entry to `SLASH_COMMANDS` whose `run` switches the menu to `"skill"` mode and loads skills via `ipc.listSkills()` into a `skills` state. Add `filteredSkills` (filter by `query`) and render them like the slash list (name + `preview` hint). Selecting a skill calls `ipc.setActiveSkill(name)`, shows a brief confirmation (e.g. the existing active-skill indicator if any, or a toast), and closes the menu. Wire the existing arrow/Enter/Esc handlers to the `skill` mode (extend `itemCount` and the Enter/selection switch to handle `menu === "skill"`).

- [ ] **Step 2: Write the spec test (C-skill)**

In `chat-behaviors.spec.ts`:
```ts
test("C-skill: /skill opens a skill picker and selecting sets the active skill", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("/skill");
  // assert the skill list shows seeded skills
  await expect(page.getByText("reviewer")).toBeVisible();
  // select it (discover the real option element / keyboard Enter)
  await page.getByText("reviewer").click();
  // assert a confirmation/active-skill indicator appears (discover the real affordance)
});
```
Discover the real selection affordance + active-skill confirmation; assert the meaningful post-select state (skill shown active). If there's no visible active-skill indicator, add a minimal one (e.g. a small "skill: reviewer" chip near the composer) and assert that — note it in the report.

- [ ] **Step 2b: Run + iterate**

Run: `cd crates/zanto-desktop && pnpm test:ui -- tests/ui/chat-behaviors.spec.ts`. Green when the picker shows skills and selecting sets the active skill. Confirm full `pnpm test:ui` + `pnpm check`.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/Composer.svelte crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts
git commit -m "feat(desktop/ui): /skill command in composer with picker + keyboard (#4)"
```

---

### Task 3: #2 — full keyboard nav + path-segment autocomplete in the `@` picker

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/Composer.svelte`
- Test: `crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts` (extend the C-8 test)

**Interfaces:** Consumes the seeded `browse_dir` mock (returns a dir + a file).

- [ ] **Step 1: Add keyboard ascend + path-segment autocomplete**

Read the picker's current handlers. Add/confirm:
- **Descend by keyboard:** arrow-select a directory + Enter descends (today `selectEntry` pushes `dirStack` for dirs — confirm Enter routes a highlighted dir through `selectEntry`). 
- **Ascend by keyboard:** a key (e.g. Backspace when `query` is empty, or `ArrowLeft`) pops `dirStack` and reloads the parent. 
- **Path-segment autocomplete:** when the `@`-query contains `/` (e.g. `@src/comp`), treat the segment(s) before the last `/` as directories to descend into (via `browseDir`) and filter the listing by the trailing segment. Keep it robust: only descend when the leading segments match real entries; otherwise just filter.
Keep arrow Up/Down + Esc behavior intact.

- [ ] **Step 2: Extend the C-8 test for keyboard nav**

In the C-8 `@`-tag test, after the picker opens, drive it by keyboard: ArrowDown to a directory, Enter to descend (assert the breadcrumb/header updates to the dir path), then the ascend key to go back (assert it returns to the roots). Insert a file via Enter and assert the `@<path>` token. Adjust to the seeded `browse_dir` entries (a dir + a file) — if the seed has only flat entries, extend the mock `browse_dir` to return a dir whose path, when passed back to `browse_dir`, yields a child file (so descend is observable).

- [ ] **Step 2b: Run + iterate**

Run: `pnpm test:ui -- tests/ui/chat-behaviors.spec.ts`. Green when keyboard descend/ascend + token insertion work. Confirm full `pnpm test:ui` + `pnpm check`.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/Composer.svelte crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts crates/zanto-desktop/src/lib/mock/backend.ts
git commit -m "feat(desktop/ui): keyboard descend/ascend + path autocomplete in @ picker (#2)"
```

---

### Task 4: #12 — Enter-to-advance/submit in the HITL ask form

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/HitlForm.svelte`
- Test: `crates/zanto-desktop/tests/ui/` (new `hitl.spec.ts` or extend an existing file)

**Interfaces:** Consumes the `hitl form` scenario (Task 1).

- [ ] **Step 1: Add Enter handling**

In HitlForm's keydown handler, when Enter is pressed inside a text/select field (not Shift+Enter, and not while a Select popover is open capturing Enter): if not the last step, advance to the next step (the existing Next action); if the last step, submit (the existing Submit action). Leave checkboxes and Shift+Enter to default behavior. Keep the existing Tab focus-trap and Esc-to-close unchanged.

- [ ] **Step 2: Write the HITL keyboard test (H-key)**

```ts
import { test, expect } from "@playwright/test";
test("H-key: HITL form advances and submits with Enter", async ({ page }) => {
  await page.goto("/");
  const composer = page.getByRole("textbox").first();
  await composer.fill("hitl form please");
  await composer.press("Enter");
  // The mock emits a 2-step form.
  await expect(page.getByText("Project name")).toBeVisible();
  await page.getByRole("textbox").last().fill("zanto"); // the form field (discover the real locator)
  await page.keyboard.press("Enter"); // advance to step 2
  await expect(page.getByText("Language")).toBeVisible();
  // complete + Enter to submit; assert the form closes
  // ... select option, Enter ...
  await expect(page.getByText("Language")).toHaveCount(0);
});
```
Discover the real field locators and the select interaction; assert the form advances on Enter and closes on final Enter (submitted). Avoid fixed sleeps.

- [ ] **Step 2b: Run + iterate**

Run: `pnpm test:ui -- tests/ui/<hitl-spec>`. Green when Enter advances then submits. Confirm full `pnpm test:ui` + `pnpm check`.

- [ ] **Step 3: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/HitlForm.svelte crates/zanto-desktop/tests/ui/
git commit -m "feat(desktop/ui): Enter advances/submits the HITL ask form (#12)"
```

---

## Self-Review

**Spec coverage:** #4 → Task 2; #2 → Task 3; #12 → Task 4; enabling mock infra → Task 1. Covered.

**Placeholder scan:** Component-internal selectors/handlers are delegated to implementers (read the real files) — consistent with prior UI batches; each task states the behavior + a concrete test. Mock handler/scenario code is concrete.

**Consistency:** Menu mode `"skill"` extends the existing `Menu` union; `list_skills`/`set_active_skill`/`respond` mock handlers match the `ipc.ts` command names; the `hitl form` scenario's `interaction_request` payload matches the `InteractionRequest` type in `ipc.ts` (`id`/`kind`/`title`/`steps`/`fields`).

**Risk:** #2 path-autocomplete is the fuzziest — scope it conservatively (descend only when leading segments match real entries; otherwise just filter) to avoid surprising the user mid-type. #12 must not hijack Enter while a Select popover is open (let the popover consume it). The existing focus-trap/Esc must remain intact.
