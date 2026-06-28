# Batch A — UI polish (layout/CSS quick wins) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development. Steps use `- [ ]`.

**Goal:** Fix four low-risk presentational findings from smoke testing: #8 link contrast on the user bubble, #11 canvas vertical scroll clipping finance cards, #13 finance tab bar horizontal overflow, and #1 absolute-path subtitle under file/dir entries (via a shared `FileListItem`).

**Architecture:** Frontend-only changes under `crates/zanto-desktop/src/`. Verify via the existing Playwright mock harness (`pnpm test:ui`, `vite --mode mock`) asserting computed styles / rendered text where deterministic; aesthetic confirmation is the user's visual pass.

**Tech Stack:** Svelte 5, Tailwind, `@playwright/test`, `pnpm check`.

## Global Constraints

- Frontend only; no Rust/IPC changes. No new dependency.
- No behavioral/logic changes beyond layout + a presentational shared component. Keep `ipc.ts` and the IPC surface unchanged.
- Verify gates after each task: `cd crates/zanto-desktop && pnpm check` (0 errors) and `pnpm test:ui` (all pass). Add Playwright assertions where deterministic; note visual-only checks for the user.
- Discover exact current markup/lines by reading the components (the survey line numbers are guides, not guarantees). Do not change unrelated code.

---

### Task 1: Shared `FileListItem` + absolute-path subtitle (#1)

**Files:**
- Create: `crates/zanto-desktop/src/lib/components/FileListItem.svelte`
- Modify: `crates/zanto-desktop/src/lib/components/Composer.svelte` (the `@`-picker list items, ~lines 479–500)
- Modify: `crates/zanto-desktop/src/lib/apps/finance/Import.svelte` (dir/file lists, ~252–277)
- Modify: `crates/zanto-desktop/src/lib/apps/finance/ResourcesPanel.svelte` (dir/file lists, ~113–136)
- Test: `crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts` (extend the existing C-8 @-tag test)

**Interfaces:**
- Produces: `FileListItem.svelte` with props `{ name: string; path: string; isDir: boolean }` rendering the name and, beneath it, the absolute `path` as a small muted monospace subtitle (truncated). Purely presentational — the parent keeps the click/keyboard handlers and wraps `FileListItem` in its existing button/`<a>`.

- [ ] **Step 1: Create `FileListItem.svelte`**

Render an icon-agnostic two-line cell (name on top, abs path subtitle below). Example:
```svelte
<script lang="ts">
  let { name, path, isDir = false }: { name: string; path: string; isDir?: boolean } = $props();
</script>
<span class="flex min-w-0 flex-col">
  <span class="truncate font-mono">{name}{isDir ? "/" : ""}</span>
  <span class="truncate text-xs text-muted-foreground/70 font-mono" title={path}>{path}</span>
</span>
```
(Match the surrounding components' class conventions; the parent supplies the icon + button wrapper.)

- [ ] **Step 2: Use it in the three list sites**

In Composer's `@`-picker item, finance `Import.svelte`, and `ResourcesPanel.svelte`, replace the inline `{name}`-only label with `<FileListItem name={...} path={...} isDir={...} />`, preserving each parent's existing button/anchor, icon, click and keyboard handlers. Keep `max-h`/scroll on the menu container; bump it only if needed for the taller rows.

- [ ] **Step 3: Extend the C-8 picker test to assert the path subtitle**

In `tests/ui/chat-behaviors.spec.ts`, in the C-8 `@`-tag test, after the autocomplete list appears, assert an item shows its absolute path (the `browse_dir` mock seeds entries like `/home/user/project/README.md`):
```ts
await expect(page.getByText("/home/user/project/README.md", { exact: false })).toBeVisible();
```
(Adjust to the actual seeded `browse_dir` paths in `src/lib/mock/backend.ts`.)

- [ ] **Step 4: Verify**

Run: `cd crates/zanto-desktop && pnpm check && pnpm test:ui`. Expected: 0 errors; all specs pass incl. the extended C-8.

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/FileListItem.svelte \
  crates/zanto-desktop/src/lib/components/Composer.svelte \
  crates/zanto-desktop/src/lib/apps/finance/Import.svelte \
  crates/zanto-desktop/src/lib/apps/finance/ResourcesPanel.svelte \
  crates/zanto-desktop/tests/ui/chat-behaviors.spec.ts
git commit -m "feat(desktop/ui): file/dir entries show absolute path subtitle (#1) via shared FileListItem"
```

---

### Task 2: Link contrast on the user bubble (#8)

**Files:**
- Modify: `crates/zanto-desktop/src/app.css` (the `.prose-zanto a` rule, ~211–215)
- Modify (if a context hook is needed): `crates/zanto-desktop/src/lib/components/Message.svelte` (user bubble wrapper, ~228–235)
- Test: `crates/zanto-desktop/tests/ui/regression-chat.spec.ts` or `chat-behaviors.spec.ts` (reuse the `link` scenario)

**Interfaces:**
- Consumes: the existing `link` mock scenario (renders a markdown link). Produces: links inside a user bubble are legible (not `var(--primary)` on a primary background).

- [ ] **Step 1: Make user-bubble links inherit a contrasting color**

The user bubble is `bg-primary text-primary-foreground`; `.prose-zanto a { color: var(--primary) }` makes links the same hue as the bg. Add a context-scoped rule so links in the user bubble use the bubble's foreground (or a bright accent), e.g. add a `data-role="user"` (or reuse an existing class) on the bubble's prose container and:
```css
[data-role="user"] .prose-zanto a { color: var(--primary-foreground); text-decoration: underline; }
```
Discover the actual user-bubble wrapper in `Message.svelte`; prefer reusing an existing distinguishing class over adding markup. Keep assistant-bubble link styling unchanged.

- [ ] **Step 2: Assert legibility in a test**

Reuse the `link` scenario but as a USER-side render isn't possible (links come from the assistant). Instead assert the CSS rule's effect deterministically: render an assistant link (existing C-12) is unaffected, and add a targeted check that the user-bubble link color computed style differs from the bubble background. If a user message never contains a rendered link in practice, this is primarily a visual fix — in that case assert the CSS selector exists/applies via a minimal DOM probe and flag the rest for the user's visual pass. Document which you did.

- [ ] **Step 3: Verify**

Run: `cd crates/zanto-desktop && pnpm check && pnpm test:ui`. Expected: green; assistant-link behavior (C-12) unchanged.

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/app.css crates/zanto-desktop/src/lib/components/Message.svelte
git commit -m "fix(desktop/ui): legible link color on the user message bubble (#8)"
```

---

### Task 3: Canvas vertical scroll (#11) + finance tabs horizontal scroll (#13)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/Canvas.svelte` (panel container, ~27)
- Modify: `crates/zanto-desktop/src/lib/apps/finance/Dashboard.svelte` (root flex/overflow ~273; tablist ~321–325)
- Test: `crates/zanto-desktop/tests/ui/regression-finance.spec.ts` (extend the R-6 finance flow)

**Interfaces:**
- Consumes: the finance app mounts in the canvas (existing mock). Produces: the canvas content area scrolls vertically; the finance tablist scrolls horizontally when it overflows.

- [ ] **Step 1: Fix the canvas vertical-scroll flex chain (#11)**

Make the canvas content scroll: give the canvas panel container a proper `min-h-0` flex chain with `overflow-y-auto` on the scrolling child (so tall finance content scrolls instead of clipping). Read `Canvas.svelte` + `Dashboard.svelte` and apply the minimal `overflow-auto` + `min-h-0`/`flex-1` fix on the right container(s). Do not change card content.

- [ ] **Step 2: Fix the finance tablist horizontal overflow (#13)**

On the finance tab bar container, add `overflow-x-auto` (keep it a single scrollable row; add `shrink-0` to tab buttons so they don't compress). Discover the exact tablist element in `Dashboard.svelte`.

- [ ] **Step 3: Assert scrollability deterministically**

In `regression-finance.spec.ts`, after mounting finance, assert computed styles (robust, no need to force overflow):
```ts
const canvasOverflow = await page.locator("<canvas-scroll-selector>").evaluate(el => getComputedStyle(el).overflowY);
expect(["auto", "scroll"]).toContain(canvasOverflow);
const tabsOverflow = await page.locator("<tablist-selector>").evaluate(el => getComputedStyle(el).overflowX);
expect(["auto", "scroll"]).toContain(tabsOverflow);
```
Discover the real selectors; prefer role/structure over brittle class chains where possible.

- [ ] **Step 4: Verify**

Run: `cd crates/zanto-desktop && pnpm check && pnpm test:ui`. Expected: green incl. the extended finance test.

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/Canvas.svelte \
  crates/zanto-desktop/src/lib/apps/finance/Dashboard.svelte \
  crates/zanto-desktop/tests/ui/regression-finance.spec.ts
git commit -m "fix(desktop/ui): canvas vertical scroll (#11) + finance tab horizontal scroll (#13)"
```

---

## Self-Review

**Spec coverage:** #1 → Task 1; #8 → Task 2; #11 + #13 → Task 3. All four Batch-A findings covered.

**Placeholder scan:** Selectors/exact lines are delegated to implementers (discovered by reading components — environment facts), consistent with prior UI batches; the approach + verification per task is concrete. The CSS-contrast (#8) and scroll (#11/#13) tests assert computed styles (deterministic) rather than pixel rendering; aesthetic confirmation is explicitly the user's visual pass.

**Consistency:** `FileListItem` props `{name,path,isDir}` consistent across the three call sites. No IPC/Rust touched. Existing tests (C-8, C-12, R-6) extended, not replaced.

**Risk:** Lowest-risk batch (presentational). Main watch-item: the canvas `min-h-0` flex chain must not break the existing two-pane resizable layout — verify the chat pane still renders and the existing specs pass.
