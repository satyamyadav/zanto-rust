# Chat Presentation Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Polish the zanto chat turn presentation toward daisyUI's calm and Claude's subtle tool/thinking style — soft tonal status badges, receding card chrome, and two chart bug fixes — without a new dependency or any feature change.

**Architecture:** Pure CSS-class + design-token edits. New soft tonal tokens are added to `app.css` and consumed via Tailwind utility classes (`bg-success-soft` etc.). The collapsible behavior, ARIA, focus rings, and all derivation logic stay byte-identical; only background/border/text color utilities change. Chart fixes are config-only additions to the ApexCharts options object.

**Tech Stack:** SvelteKit 5 (runes), Tailwind v4 (`@theme inline` tokens, oklch), bits-ui components, ApexCharts, Tauri. Package manager: `pnpm`.

## Global Constraints

- No new npm/cargo dependency. Borrow daisyUI's *look* only.
- No change to IPC, stores, session logic, or the `items`/`thinkingText`/`stepCount` derivations in `Message.svelte`.
- Every `focus-visible:ring-*` and `aria-expanded`/`aria-selected` attribute preserved exactly.
- Collapse/expand state and bodies (`open`, `cardOpen`, `argsOpen`, `outputOpen`, `{#if open}`) unchanged.
- Verify visually in **both** light and dark themes.
- Reduced-motion handling in `app.css` stays untouched.
- Frontend dir for all commands: `crates/zanto-desktop`.
- Verification is the dev build (`pnpm check`) + screenshot, NOT `cargo`.

---

## File Structure

- `crates/zanto-desktop/src/app.css` — add 6 soft tonal tokens (3 pairs) per theme + `@theme inline` wiring. (Task 1)
- `crates/zanto-desktop/src/lib/components/segments/ToolCallSegment.svelte` — soft badges + receding chrome. (Task 2)
- `crates/zanto-desktop/src/lib/components/segments/WorkflowGroup.svelte` — soft badge + receding chrome. (Task 3)
- `crates/zanto-desktop/src/lib/components/segments/ThinkingBlock.svelte` — receding chrome. (Task 4)
- `crates/zanto-desktop/src/lib/blocks/Chart.svelte` — y-axis formatter + donut legend. (Task 5)
- `crates/zanto-desktop/src/lib/components/Composer.svelte` — chip fill consistency. (Task 6)

Tasks are ordered so the token foundation (Task 1) lands before its consumers (Tasks 2–4). Tasks 5 and 6 are independent and may run in any order after Task 1.

---

### Task 1: Soft tonal tokens

**Files:**
- Modify: `crates/zanto-desktop/src/app.css` (`:root` ~line 17, `.dark` ~line 57, `@theme inline` ~line 95)

**Interfaces:**
- Produces: CSS custom properties `--success-soft`, `--success-soft-foreground`, `--warning-soft`, `--warning-soft-foreground`, `--destructive-soft`, `--destructive-soft-foreground`, and matching `--color-*-soft` theme tokens. Tasks 2 and 3 consume these as Tailwind classes `bg-success-soft`, `text-success-soft-foreground`, `bg-warning-soft`, `text-warning-soft-foreground`, `bg-destructive-soft`, `text-destructive-soft-foreground`.

- [ ] **Step 1: Add light-theme soft tokens**

In `:root`, after the existing `--destructive-foreground` / `--success` / `--warning` block (right after line 41, before `--border`), insert:

```css
  --success-soft: oklch(0.95 0.04 150);
  --success-soft-foreground: oklch(0.45 0.11 150);
  --warning-soft: oklch(0.95 0.05 75);
  --warning-soft-foreground: oklch(0.48 0.12 70);
  --destructive-soft: oklch(0.95 0.04 27);
  --destructive-soft-foreground: oklch(0.51 0.19 27);
```

- [ ] **Step 2: Add dark-theme soft tokens**

In `.dark`, after the existing `--warning-foreground` line (right after line 79, before `--border`), insert:

```css
  --success-soft: oklch(0.32 0.05 150);
  --success-soft-foreground: oklch(0.82 0.13 150);
  --warning-soft: oklch(0.34 0.06 75);
  --warning-soft-foreground: oklch(0.85 0.13 80);
  --destructive-soft: oklch(0.33 0.06 27);
  --destructive-soft-foreground: oklch(0.82 0.15 27);
```

- [ ] **Step 3: Wire tokens into `@theme inline`**

In the `@theme inline` block, after the existing `--color-warning-foreground` line (~line 124, before `--color-border`), insert:

```css
  --color-success-soft: var(--success-soft);
  --color-success-soft-foreground: var(--success-soft-foreground);
  --color-warning-soft: var(--warning-soft);
  --color-warning-soft-foreground: var(--warning-soft-foreground);
  --color-destructive-soft: var(--destructive-soft);
  --color-destructive-soft-foreground: var(--destructive-soft-foreground);
```

- [ ] **Step 4: Verify the build compiles the new tokens**

Run (from `crates/zanto-desktop`): `pnpm check`
Expected: completes with no new errors. (`pnpm check` runs `svelte-check`; CSS token additions don't affect it, but this confirms nothing in `app.css` broke parsing.)

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src/app.css
git commit -m "feat(desktop): add soft tonal status tokens"
```

---

### Task 2: ToolCallSegment — soft badges + receding chrome

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/segments/ToolCallSegment.svelte`

**Interfaces:**
- Consumes: the `bg-*-soft` / `text-*-soft-foreground` classes from Task 1.
- Produces: nothing other tasks consume.

- [ ] **Step 1: Soften the card wrapper chrome**

Find the wrapper (line ~48):

```svelte
<div class="rounded-md border border-border bg-card text-xs">
```

Replace with:

```svelte
<div class="rounded-md border border-border/60 text-xs">
```

- [ ] **Step 2: Add hover affordance to the header button**

Find the header `<button>` opening (line ~50-54). Its class currently ends:

```
class="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
```

Add `hover:bg-muted/40 transition-colors`:

```
class="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
```

- [ ] **Step 3: Swap the `running` pill to warning-soft**

Find (line ~60):

```svelte
      <span class="flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 font-display text-muted-foreground">
```

Replace with:

```svelte
      <span class="flex items-center gap-1 rounded-full bg-warning-soft px-2 py-0.5 font-display text-warning-soft-foreground">
```

- [ ] **Step 4: Swap the `ok` pill to success-soft**

Find (line ~65):

```svelte
      <span class="flex items-center gap-1 rounded-full bg-success px-2 py-0.5 font-display text-success-foreground">
```

Replace with:

```svelte
      <span class="flex items-center gap-1 rounded-full bg-success-soft px-2 py-0.5 font-display text-success-soft-foreground">
```

- [ ] **Step 5: Swap the `error` pill to destructive-soft**

Find (line ~70):

```svelte
      <span class="flex items-center gap-1 rounded-full bg-destructive px-2 py-0.5 font-display text-destructive-foreground">
```

Replace with:

```svelte
      <span class="flex items-center gap-1 rounded-full bg-destructive-soft px-2 py-0.5 font-display text-destructive-soft-foreground">
```

- [ ] **Step 6: Verify build**

Run (from `crates/zanto-desktop`): `pnpm check`
Expected: no new errors.

- [ ] **Step 7: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/segments/ToolCallSegment.svelte
git commit -m "style(desktop): soft tonal tool-call badges + receding card chrome"
```

---

### Task 3: WorkflowGroup — soft badge + receding chrome

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/segments/WorkflowGroup.svelte`

**Interfaces:**
- Consumes: the `bg-*-soft` / `text-*-soft-foreground` classes from Task 1.

- [ ] **Step 1: Soften the card wrapper chrome**

Find (line ~24):

```svelte
<div class="rounded-md border border-border bg-card">
```

Replace with:

```svelte
<div class="rounded-md border border-border/60">
```

- [ ] **Step 2: Add hover affordance to the header button**

Find the header `<button>` (line ~25-29); its class ends:

```
class="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
```

Replace with:

```
class="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-xs transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
```

- [ ] **Step 3: Swap the three pills to soft tonal**

Find the pill block (lines ~34-40):

```svelte
    {#if pill === "error"}
      <span class="ml-auto rounded-full bg-destructive px-2 py-0.5 font-mono text-destructive-foreground">{done}/{total} done</span>
    {:else if pill === "done"}
      <span class="ml-auto rounded-full bg-success px-2 py-0.5 font-mono text-success-foreground">{done}/{total} done</span>
    {:else}
      <span class="ml-auto rounded-full bg-muted px-2 py-0.5 font-mono text-muted-foreground">{done}/{total} done</span>
    {/if}
```

Replace with:

```svelte
    {#if pill === "error"}
      <span class="ml-auto rounded-full bg-destructive-soft px-2 py-0.5 font-mono text-destructive-soft-foreground">{done}/{total} done</span>
    {:else if pill === "done"}
      <span class="ml-auto rounded-full bg-success-soft px-2 py-0.5 font-mono text-success-soft-foreground">{done}/{total} done</span>
    {:else}
      <span class="ml-auto rounded-full bg-warning-soft px-2 py-0.5 font-mono text-warning-soft-foreground">{done}/{total} done</span>
    {/if}
```

- [ ] **Step 4: Verify build**

Run (from `crates/zanto-desktop`): `pnpm check`
Expected: no new errors.

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/segments/WorkflowGroup.svelte
git commit -m "style(desktop): soft tonal workflow badge + receding card chrome"
```

---

### Task 4: ThinkingBlock — receding chrome

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/segments/ThinkingBlock.svelte`

**Interfaces:**
- Consumes: nothing from Task 1 (no status badge here). Receding-chrome change only.

- [ ] **Step 1: Soften the card wrapper chrome**

Find (line ~37):

```svelte
<div class="rounded-md border border-border bg-card text-xs">
```

Replace with:

```svelte
<div class="rounded-md border border-border/60 text-xs">
```

- [ ] **Step 2: Add hover affordance to the header button**

Find the header `<button>` (line ~38-44); its class ends:

```
class="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-default"
```

Replace with:

```
class="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-default disabled:hover:bg-transparent"
```

(The `disabled:hover:bg-transparent` keeps the no-text, non-interactive header from highlighting on hover.)

- [ ] **Step 3: Verify build**

Run (from `crates/zanto-desktop`): `pnpm check`
Expected: no new errors.

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/segments/ThinkingBlock.svelte
git commit -m "style(desktop): receding chrome on thinking block"
```

---

### Task 5: Chart — y-axis formatter + donut legend

**Files:**
- Modify: `crates/zanto-desktop/src/lib/blocks/Chart.svelte` (`buildOptions`, lines ~47-54)

**Interfaces:**
- Consumes: nothing. Self-contained ApexCharts config.

- [ ] **Step 1: Add bottom legend to the arc (pie/doughnut) branch**

Find (lines ~47-49):

```svelte
    if (isArc) {
      return { ...base, series: (datasets[0]?.data ?? []).map((n) => (Number.isFinite(n) ? n : 0)), labels };
    }
```

Replace with:

```svelte
    if (isArc) {
      return {
        ...base,
        series: (datasets[0]?.data ?? []).map((n) => (Number.isFinite(n) ? n : 0)),
        labels,
        legend: { position: "bottom", horizontalAlign: "center" },
      };
    }
```

- [ ] **Step 2: Add the y-axis formatter to the bar/line branch**

Find (lines ~50-54):

```svelte
    return {
      ...base,
      series: datasets.map((ds, i) => ({ name: ds.label ?? `Series ${i + 1}`, data: ds.data ?? [] })),
      xaxis: { categories: labels },
    };
```

Replace with:

```svelte
    return {
      ...base,
      series: datasets.map((ds, i) => ({ name: ds.label ?? `Series ${i + 1}`, data: ds.data ?? [] })),
      xaxis: { categories: labels },
      yaxis: {
        labels: {
          formatter: (v: number) => {
            if (!Number.isFinite(v)) return "";
            const abs = Math.abs(v);
            if (abs >= 1000) return `${(v / 1000).toFixed(abs % 1000 === 0 ? 0 : 1)}k`;
            return Number.isInteger(v) ? String(v) : String(Math.round(v * 100) / 100);
          },
        },
      },
    };
```

- [ ] **Step 3: Verify build**

Run (from `crates/zanto-desktop`): `pnpm check`
Expected: no new errors. (The `formatter` param is typed `number`; confirm svelte-check accepts it. If it flags the `base` record's `unknown` typing, leave the annotation as `(v: number)` — ApexCharts passes a number at runtime.)

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/blocks/Chart.svelte
git commit -m "fix(desktop): chart y-axis number formatter + bottom donut legend"
```

---

### Task 6: Composer — chip fill consistency

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/Composer.svelte`

**Interfaces:**
- Consumes: nothing. Cosmetic fill change only. No behavior change.

- [ ] **Step 1: Soften the paste chip fill**

Find the paste chip span (line ~544-546):

```svelte
        <span
          class="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted px-2 py-1 text-xs text-muted-foreground"
        >
```

Replace `bg-muted` with `bg-muted/60`:

```svelte
        <span
          class="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted/60 px-2 py-1 text-xs text-muted-foreground"
        >
```

- [ ] **Step 2: Soften the attachment chip fill**

Find the attachment chip span (line ~560-563):

```svelte
        <span
          class="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted px-2 py-1 text-xs text-muted-foreground"
          title={a.path}
        >
```

Replace `bg-muted` with `bg-muted/60`:

```svelte
        <span
          class="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted/60 px-2 py-1 text-xs text-muted-foreground"
          title={a.path}
        >
```

- [ ] **Step 3: Soften the active-skill chip fill**

Find the active-skill chip span (line ~593-595):

```svelte
      <span
        class="inline-flex items-center gap-1 rounded-md border border-border bg-muted px-1.5 py-0.5 text-xs text-muted-foreground"
        aria-label="Active skill: {activeSkillName}"
      >
```

Replace `bg-muted` with `bg-muted/60`:

```svelte
      <span
        class="inline-flex items-center gap-1 rounded-md border border-border bg-muted/60 px-1.5 py-0.5 text-xs text-muted-foreground"
        aria-label="Active skill: {activeSkillName}"
      >
```

- [ ] **Step 4: Verify build**

Run (from `crates/zanto-desktop`): `pnpm check`
Expected: no new errors.

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/Composer.svelte
git commit -m "style(desktop): soften composer chip fills for consistency"
```

---

### Task 7: Visual verification pass (both themes)

**Files:** none modified. This task is the quality gate from the spec.

- [ ] **Step 1: Launch the dev app**

Run (from `crates/zanto-desktop`): `pnpm tauri dev` (or `pnpm dev` for the browser-only frontend if Tauri is not needed for visuals).
Open a chat session that has: an assistant turn with a Thinking block, an expanded tool call, a Workflow group, and a chart (the "spending summary" flow in the screenshots reproduces all of these).

- [ ] **Step 2: Verify in dark theme**

Confirm against the spec's verification checklist:
- Tool-call badges: running=amber-soft, done=green-soft, error=red-soft.
- Workflow badge soft in all three states.
- Collapsed thinking/tool/workflow rows are quiet (no card fill); hovering a header shows the `bg-muted/40` highlight.
- Expanded rows still legible with their `border-t` dividers.
- Chart y-axis shows `8k` / `6k`, NOT `8000.0000000000000`.
- Donut legend sits at the bottom and no longer collides with the pin icon.

- [ ] **Step 3: Verify in light theme**

Toggle Settings → Appearance → Light. Repeat the Step 2 checks. Pay attention to soft-badge contrast — if any badge text is hard to read on its tint, nudge the relevant `*-soft-foreground` lightness down (light theme) in `app.css` and re-commit Task 1's file.

- [ ] **Step 4: Verify behavior is unchanged**

- Click each collapsed row → it expands; click again → collapses.
- Tool-call args/output sub-toggles still work.
- Message copy button still works.
- Keyboard: Tab to a header, confirm the focus ring shows; Enter toggles.

- [ ] **Step 5: Commit any contrast tweaks (if Step 3 required them)**

```bash
git add crates/zanto-desktop/src/app.css
git commit -m "style(desktop): tune soft-badge contrast after visual pass"
```

If no tweaks were needed, skip this commit.

---

## Self-Review

**Spec coverage:**
- §1 tokens → Task 1. ✓
- §2 soft badges (ToolCallSegment + WorkflowGroup) → Tasks 2, 3. ✓
- §3 receding chrome (all three components) → Tasks 2, 3, 4. ✓
- §4a y-axis formatter → Task 5 Step 2. ✓
- §4b donut legend → Task 5 Step 1. ✓
- §5 composer chips → Task 6. ✓
- Quality floor / verification checklist → Task 7. ✓

**Placeholder scan:** No TBD/TODO; every code step shows exact before/after. ✓

**Type/name consistency:** Token names (`--success-soft`, `--warning-soft`, `--destructive-soft` + `-foreground` variants) are defined in Task 1 and consumed verbatim as `bg-*-soft` / `text-*-soft-foreground` in Tasks 2–3. The `running`/in-progress state maps to **warning-soft** consistently in both Task 2 (Step 3) and Task 3 (Step 3). ✓
