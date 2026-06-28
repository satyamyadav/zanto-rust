# Settings Two-Pane Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Redesign the Settings dialog into a two-pane shell (left grouped nav + right content pane) with provider cards and theme swatch cards, taking the visual structure from the user's Orbit mockups — without changing any IPC/store/form logic.

**Architecture:** A single component, `SettingsDialog.svelte`, is restructured. The `<script>` block keeps every existing `$state`/`$derived`/`$effect`/handler and gains only: a `section` selector state, a `NAV` descriptor array, a `SectionId` type, and an `avatarTint()` helper. The template changes from one scrolling column into a `flex` row — a left nav and a right pane that renders only the active section. Each section's controls move **verbatim** into a section wrapper; only two controls are replaced (provider dropdown → cards, theme buttons → swatches).

**Tech Stack:** Svelte 5 (runes), Tailwind v4 (oklch tokens), bits-ui Dialog/Select/Input/Button, mode-watcher (`mode`/`setMode`), Lucide icons. Package manager: `pnpm`. Frontend dir: `crates/zanto-desktop`.

## Global Constraints

- **Presentational only.** No IPC, Rust, store, or config-schema change. Every `ipc.*`/`appStore`/`refreshConfig` call and the seed `$effect` (the `untrack` block) stay byte-identical.
- No new dependency.
- No new themes (Paper/Midnight map to existing light/dark; Slate/Solar NOT built).
- No provider enable/disable toggles and no "Test connection" action (no backend exists for them).
- Density displayed labels stay "Compact / Normal / Relaxed"; density logic unchanged.
- Preserve every `focus-visible:ring-*`, add `aria-pressed`/`aria-current` on new nav/cards.
- Verify both light + dark themes; `pnpm check` 0/0; `pnpm test:ui` green.
- All commands run from `crates/zanto-desktop`. The verification gate is `pnpm check` (no unit test for this component); visual confirmation via `pnpm dev:mock` on port 1430.

## File Structure

- `crates/zanto-desktop/src/lib/components/SettingsDialog.svelte` — the only file changed. All tasks modify it.

Because it's one component, tasks are sequenced by concern so each leaves the dialog in a working, reviewable state:
1. Script additions (nav model, section state, tint helper) — no template change yet.
2. Two-pane shell + section wrappers (move existing markup verbatim into panes).
3. Provider cards (replace dropdown).
4. Theme swatch cards (replace light/dark buttons).
5. Section headers (title + description) + esc/Close affordance + polish.

---

### Task 1: Script — section state, nav model, tint helper

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/SettingsDialog.svelte` (`<script>` block only)

**Interfaces:**
- Produces: `type SectionId`, `section` state, `NAV` array, `avatarTint(id)` — all consumed by Tasks 2–5.

- [ ] **Step 1: Add the imports for nav icons**

In the import block (after the existing `FolderPlusIcon` import, ~line 15), add the Lucide icons the nav will use:

```svelte
  import CpuIcon from "@lucide/svelte/icons/cpu";
  import PaletteIcon from "@lucide/svelte/icons/palette";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import SlidersIcon from "@lucide/svelte/icons/sliders-horizontal";
  import BookOpenIcon from "@lucide/svelte/icons/book-open";
  import LayersIcon from "@lucide/svelte/icons/layers";
```

- [ ] **Step 2: Add section state + nav descriptor + tint helper**

At the end of the `<script>` block (after `let showOverrides = $state(false);`, ~line 263), add:

```svelte
  // ── Two-pane nav ──────────────────────────────────────────────────────────
  // Which section the right pane shows. Pure presentation; all form state below
  // is shared across sections and persists when switching.
  type SectionId = "providers" | "theme" | "folders" | "context" | "generation" | "skill";
  let section = $state<SectionId>("providers");

  // Grouped nav: heading → items (id, label, icon component). Rendered in the
  // left sidebar. Order mirrors the previous single-column section order.
  const NAV: { heading: string; items: { id: SectionId; label: string; icon: typeof CpuIcon }[] }[] = [
    { heading: "Models", items: [{ id: "providers", label: "Providers", icon: CpuIcon }] },
    {
      heading: "App",
      items: [
        { id: "theme", label: "Theme", icon: PaletteIcon },
        { id: "folders", label: "Folder access", icon: FolderIcon },
        { id: "context", label: "Context", icon: LayersIcon },
        { id: "generation", label: "Generation", icon: SlidersIcon },
        { id: "skill", label: "Skill", icon: BookOpenIcon },
      ],
    },
  ];

  // Deterministic avatar tint for a provider id: hash → hue, rendered as an
  // oklch background. Stable per id, no hardcoded brand colors.
  function avatarTint(id: string): string {
    let h = 0;
    for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) % 360;
    return `oklch(0.65 0.15 ${h})`;
  }
```

- [ ] **Step 3: Verify build**

Run: `pnpm check`
Expected: completes with 0 errors. (New script symbols are unused-but-valid until the template uses them in later tasks; svelte-check does not error on declared-unused module-level `const`/`function`. If it flags `section` as unused, that's fine — Task 2 consumes it; if check ERRORS rather than warns, stop and report.)

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/SettingsDialog.svelte
git commit -m "feat(desktop): settings nav model + section state + avatar tint"
```

---

### Task 2: Two-pane shell + section wrappers (verbatim move)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/SettingsDialog.svelte` (template)

**Interfaces:**
- Consumes: `section`, `NAV` from Task 1.
- Produces: the shell + `{#if section === "..."}` wrappers consumed by Tasks 3–5.

This is the highest-risk task: the existing 7 `<section>` blocks (lines ~272–531) move into per-section conditional wrappers. **Move the inner markup verbatim** — do not edit any binding, handler, or control inside. Only the wrapping container and visibility change.

- [ ] **Step 1: Replace the dialog shell + section container**

Replace the current `<Dialog.Content>` opening through the closing `</Dialog.Content>` (lines ~267–532). The new structure: a flex row with a left nav and a right pane. Inside the right pane, EACH existing `<section>`'s INNER content is wrapped in `{#if section === "<id>"}`.

Replace this opening:

```svelte
  <Dialog.Content class="sm:max-w-[80vw] max-h-[80vh] flex flex-col">
    <Dialog.Header>
      <Dialog.Title class="font-display">Settings</Dialog.Title>
    </Dialog.Header>

    <div class="min-h-0 flex-1 space-y-6 overflow-y-auto py-1 pr-1">
```

with:

```svelte
  <Dialog.Content class="sm:max-w-[860px] h-[80vh] p-0 gap-0 overflow-hidden flex flex-row">
    <!-- Left nav -->
    <nav class="flex w-[190px] shrink-0 flex-col border-r border-border bg-sidebar p-3" aria-label="Settings sections">
      <p class="px-2 pb-3 font-display text-sm font-semibold">Settings</p>
      <div class="flex flex-1 flex-col gap-4 overflow-y-auto">
        {#each NAV as group (group.heading)}
          <div class="flex flex-col gap-0.5">
            <p class="px-2 pb-1 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">{group.heading}</p>
            {#each group.items as item (item.id)}
              <button
                type="button"
                aria-current={section === item.id ? "page" : undefined}
                onclick={() => (section = item.id)}
                class="flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {section === item.id
                  ? 'bg-accent text-accent-foreground'
                  : 'text-muted-foreground hover:bg-muted/40 hover:text-foreground'}"
              >
                <item.icon class="size-4 shrink-0" />
                {item.label}
              </button>
            {/each}
          </div>
        {/each}
      </div>
      <button
        type="button"
        onclick={() => (open = false)}
        class="mt-3 flex items-center justify-between rounded-md px-2 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-muted/40 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        Close
        <kbd class="rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px]">esc</kbd>
      </button>
    </nav>

    <!-- Right pane -->
    <div class="min-h-0 flex-1 overflow-y-auto p-6">
```

- [ ] **Step 2: Wrap each existing section in its `{#if}` and remove the old `<section>` wrappers**

The right pane now contains the 6 sections, each gated by `section`. Take the existing inner content of each `<section>` block and wrap it. Concretely:

- The **Provider & model** `<section>` (lines ~275–407) → `{#if section === "providers"}<div class="space-y-3"> …existing inner… </div>{/if}`
- The **Appearance** `<section>` (lines ~410–444) → `{#if section === "theme"}<div class="space-y-3"> …existing inner… </div>{/if}`
- The **Folder access** `<section>` (lines ~447–466) → `{#if section === "folders"}<div class="space-y-3"> …existing inner… </div>{/if}`
- The **Context** `<section>` (lines ~469–491) → `{#if section === "context"}<div class="space-y-3"> …existing inner… </div>{/if}`
- The **Generation** `<section>` (lines ~494–503) → `{#if section === "generation"}<div class="space-y-3"> …existing inner… </div>{/if}`
- The **Skill** `<section>` (lines ~506–530) → `{#if section === "skill"}<div class="space-y-3"> …existing inner… </div>{/if}`

For EACH: keep the existing `<h3>` heading and ALL inner controls exactly as they are right now (do not alter bindings/handlers). Only the `<section class="space-y-3">` wrapper becomes `{#if section === "<id>"}<div class="space-y-3">…</div>{/if}`.

- [ ] **Step 3: Close the right pane and dialog**

After the last section's `{/if}`, close the structures:

```svelte
    </div>
  </Dialog.Content>
```

(Replaces the old `</div>` + `</Dialog.Content>`. The old `<Dialog.Header>` is gone — the nav now carries the "Settings" title. The old `Dialog.Content` `flex flex-col` column is replaced by the `flex flex-row` shell.)

- [ ] **Step 4: Verify build**

Run: `pnpm check`
Expected: 0 errors. If `Dialog.Header`/`Dialog.Title` are now unused imports, that's a svelte-check *warning* not an error — but to keep output pristine, remove the `Dialog.Header`/`Dialog.Title` usage only (the `import * as Dialog` stays, since `Dialog.Root`/`Dialog.Content` are still used). No separate import line to delete.

- [ ] **Step 5: Visual smoke check**

Run: `pnpm dev:mock` (port 1430), open the app, open Settings (gear icon). Confirm: left nav lists Providers / Theme / Folder access / Context / Generation / Skill under Models/App headings; clicking each shows that section; the previous controls all render; Close + esc work. Stop the server when done.

- [ ] **Step 6: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/SettingsDialog.svelte
git commit -m "feat(desktop): two-pane settings shell with grouped nav"
```

---

### Task 3: Provider cards (replace dropdown)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/SettingsDialog.svelte` (the Providers section, inside `{#if section === "providers"}`)

**Interfaces:**
- Consumes: `registry` (existing `$derived`), `activeProvider`, `ensureProviderPatch`, `avatarTint` (Task 1).

- [ ] **Step 1: Replace the active-provider `Select` with a card list**

Inside the Providers section, find the current provider dropdown block (the `<div class="space-y-1.5">` containing `<Select.Root … value={activeProvider} …>` — lines ~278–293 in the original). Replace that entire block with:

```svelte
        <div class="space-y-1.5">
          <span class="text-xs text-muted-foreground">Active provider</span>
          <div class="flex flex-col gap-2" role="radiogroup" aria-label="Active provider">
            {#each registry as r (r.id)}
              {@const isActive = r.id === activeProvider}
              <button
                type="button"
                role="radio"
                aria-checked={isActive}
                onclick={() => { activeProvider = r.id; ensureProviderPatch(r.id); }}
                class="flex items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {isActive
                  ? 'border-primary/50 bg-accent/40'
                  : 'border-border hover:bg-muted/40'}"
              >
                <span
                  class="grid size-8 shrink-0 place-items-center rounded-md font-display text-sm font-semibold text-white"
                  style="background: {avatarTint(r.id)}"
                  aria-hidden="true"
                >
                  {r.label.slice(0, 2)}
                </span>
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-sm font-medium text-foreground">{r.label}</span>
                  <span class="block truncate font-mono text-xs text-muted-foreground">{r.default_endpoint ?? "—"}</span>
                </span>
                {#if isActive}
                  <span class="flex items-center gap-1 rounded-full bg-success-soft px-2 py-0.5 font-display text-xs text-success-soft-foreground">
                    <span class="size-1.5 rounded-full bg-success-soft-foreground"></span>
                    Active
                  </span>
                {/if}
              </button>
            {/each}
          </div>
        </div>
```

- [ ] **Step 2: Verify the Configure block still references the active provider**

No change needed below — the model/endpoint/key/overrides block already keys off `activeProvider`/`activeProviderPatch()`/`activeInfo`. Confirm by reading: the `{#if activeProvider}` block (model input, endpoint-or-key, overrides, Save changes) remains directly after the card list, unchanged.

- [ ] **Step 3: Verify build**

Run: `pnpm check`
Expected: 0 errors. `Select` may now be unused IF no other section uses it — but the **Skill** section still uses `Select.Root` (active-skill select), so the `import * as Select` stays. Confirm no unused-import warning appears; if it does, it means Skill's select was affected — stop and report.

- [ ] **Step 4: Visual check**

`pnpm dev:mock`, open Settings → Providers. Confirm: provider cards render with avatar + endpoint; the active one shows the "Active" pill and a highlighted border; clicking a different card moves the Active pill and updates the Configure fields below. Both themes. Stop server.

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/SettingsDialog.svelte
git commit -m "feat(desktop): provider selection as cards with Active pill"
```

---

### Task 4: Theme swatch cards (replace light/dark buttons)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/SettingsDialog.svelte` (the Theme section, inside `{#if section === "theme"}`)

**Interfaces:**
- Consumes: `mode`, `setMode` (existing imports from `mode-watcher`).

- [ ] **Step 1: Replace the Theme `<Button>` pair with swatch cards**

Inside the theme section, find the theme button block (the `<div class="space-y-1.5">` containing the Light/Dark `<Button>`s — the block under `id="cfg-theme-label"`). Replace that block with two swatch cards:

```svelte
        <div class="space-y-1.5">
          <span class="text-xs text-muted-foreground" id="cfg-theme-label">Theme</span>
          <div class="grid grid-cols-2 gap-3" role="radiogroup" aria-labelledby="cfg-theme-label">
            {#each [
              { id: "light", name: "Paper", desc: "Bright light theme with a violet accent.", swatch: ["#f7f7f5", "#ffffff", "#ececef", "#6d5ef0"] },
              { id: "dark", name: "Midnight", desc: "Deep dark theme with a violet accent.", swatch: ["#23232b", "#2b2b35", "#34343f", "#8b7cff"] },
            ] as t (t.id)}
              {@const isActive = mode.current === t.id}
              <button
                type="button"
                role="radio"
                aria-checked={isActive}
                onclick={() => setMode(t.id as "light" | "dark")}
                class="flex flex-col gap-2 rounded-lg border p-2.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {isActive
                  ? 'border-primary ring-1 ring-primary/40'
                  : 'border-border hover:bg-muted/40'}"
              >
                <span class="flex h-12 overflow-hidden rounded-md border border-border" aria-hidden="true">
                  {#each t.swatch as c (c)}
                    <span class="flex-1" style="background: {c}"></span>
                  {/each}
                </span>
                <span class="flex items-center justify-between">
                  <span class="text-sm font-medium text-foreground">{t.name}</span>
                  {#if isActive}<CheckIcon class="size-4 text-primary" />{/if}
                </span>
                <span class="text-xs text-muted-foreground">{t.desc}</span>
              </button>
            {/each}
          </div>
        </div>
```

- [ ] **Step 2: Add the CheckIcon import**

In the import block, add:

```svelte
  import CheckIcon from "@lucide/svelte/icons/check";
```

- [ ] **Step 3: Verify build**

Run: `pnpm check`
Expected: 0 errors. The `Button` import is still used (Save buttons, folder/context/etc.), so it stays.

- [ ] **Step 4: Visual check**

`pnpm dev:mock`, Settings → Theme. Confirm: two swatch cards (Paper/Midnight) with palette strips; the active one shows the check + accent ring; clicking flips the whole app theme live; the swatch preview strips read as light vs dark. Density control still works below. Stop server.

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/SettingsDialog.svelte
git commit -m "feat(desktop): theme swatch cards (Paper/Midnight)"
```

---

### Task 5: Section headers + description copy

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/SettingsDialog.svelte` (each section's heading)

**Interfaces:**
- Consumes: nothing new.

- [ ] **Step 1: Replace each section's bare `<h3>` with a title + description header**

Each section currently starts with `<h3 class="font-display text-sm font-semibold tracking-tight">Title</h3>`. Replace each with a header block: a larger title + a one-line muted description (per the spec's writing guidance). Use these exact strings:

- Providers: title "Providers", desc "Choose where zanto gets its intelligence. API keys are stored in your system keychain."
- Theme: title "Theme", desc "Pick a color scheme and how dense the layout feels. Changes apply instantly."
- Folder access: title "Folder access", desc "Folders the assistant may read and write. Add one to grant access."
- Context: title "Context", desc "How much conversation history is kept verbatim before older turns are summarized."
- Generation: title "Generation", desc "Defaults applied to every turn. A provider's overrides take precedence."
- Skill: title "Skill", desc "Load a markdown skill to steer how the assistant works."

Header markup (apply per section, substituting title/desc):

```svelte
          <div class="space-y-1">
            <h2 class="font-display text-lg font-semibold tracking-tight">TITLE</h2>
            <p class="text-sm text-muted-foreground">DESC</p>
          </div>
```

For the Theme section, this replaces the old `<h3>Appearance</h3>` (note: the
section id is `theme`, the displayed title is "Theme", matching the nav).

For sections that already render their own descriptive `<p>` below the heading
(Context has a long help paragraph; Generation has a description `<p>`; Skill has
an empty-state `<p>`) — keep those inner paragraphs; the new header desc is the
short summary line, the existing inner `<p>` stays as detailed help. Do not
delete existing help text.

- [ ] **Step 2: Verify build**

Run: `pnpm check`
Expected: 0 errors.

- [ ] **Step 3: Visual check (full pass, both themes)**

`pnpm dev:mock`. Walk all 6 sections in BOTH light and dark:
- Each section header shows title + description.
- Providers cards + Active pill + Configure work; switching provider updates fields.
- Theme swatches set the theme; density works.
- Folder access, Context, Generation, Skill behave exactly as before (add folder, save context, save generation, select skill).
- Nav highlight, focus rings (Tab through nav + cards), esc + Close all work.
Stop server.

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/SettingsDialog.svelte
git commit -m "feat(desktop): per-section title + description headers"
```

---

### Task 6: Regression + verification gate

**Files:** none modified.

- [ ] **Step 1: Type/build gate**

Run: `pnpm check`
Expected: `0 ERRORS 0 WARNINGS`. Warnings (e.g. an unused import left behind) must be cleaned before this passes — pristine output is the bar.

- [ ] **Step 2: UI test suite**

Run: `CI=1 pnpm test:ui`
Expected: all tests pass (the suite spins up its own dev:mock; ensure no stray dev server holds port 1430 first). The Settings dialog has no dedicated spec, but app-shell and chat tests must not regress.

- [ ] **Step 3: Confirm presentational-only**

Run: `git diff main -- crates/zanto-desktop/src/lib/components/SettingsDialog.svelte | grep -E '^\+' | grep -E 'ipc\.|refreshConfig|appStore\.|untrack|setConfig|setApiKey|clearApiKey|addAllowedPath|pickFolder|setActiveSkill|listModels|listSkills'`
Expected: NO output lines that change the logic — any matching `+` line should be an unchanged move (same call, same args). Eyeball each hit: if a handler's arguments or call shape changed, that violates the presentational-only constraint — stop and report. (Lines may appear due to relocation; what matters is they are byte-identical to their `-` counterparts.)

- [ ] **Step 4: No commit** (verification only)

If Steps 1–3 pass with no logic drift, the feature is complete.

---

## Self-Review

**Spec coverage:**
- Two-pane shell + grouped nav + esc/Close → Task 2. ✓
- Per-section title/description header → Task 5. ✓
- Provider cards + Active pill (replace dropdown) → Task 3. ✓
- Theme Paper/Midnight swatch cards + density kept → Task 4. ✓
- Folder/Context/Generation/Skill moved verbatim → Task 2. ✓
- State additions (section, NAV, SectionId, avatarTint) → Task 1. ✓
- No new themes / no toggles / no Test connection → enforced by Global Constraints; Tasks 3–4 explicitly omit them. ✓
- Density labels unchanged → Task 4 keeps the existing density block untouched. ✓
- Quality floor (both themes, focus, esc, check, test:ui) → Tasks 5–6. ✓

**Placeholder scan:** No TBD/TODO; every code step shows exact markup. ✓

**Type/name consistency:** `SectionId` union (`"providers" | "theme" | "folders" | "context" | "generation" | "skill"`) defined in Task 1, consumed identically as `section === "<id>"` in Tasks 2–5. `avatarTint(id: string): string` defined in Task 1, called in Task 3. `NAV` item ids match the `SectionId` union exactly. `mode.current`/`setMode` used in Task 4 match the existing imports. ✓

**Risk control:** Task 2 is flagged as the verbatim-move task; Task 6 Step 3 adds an explicit grep gate proving no IPC/store-call drift — the core constraint. ✓
