# zanto — chat presentation polish (design)

_Date: 2026-06-24_

## Goal

UI/UX polish of the chat turn presentation, borrowing daisyUI's calm and
Claude's subtle tool/thinking style, **without a new dependency** and **without
changing any working feature**. All changes derive from the existing
`app.css` token system. Pure CSS-class + token edits — no IPC, store, session,
or derivation logic is touched.

Direction: chat scaffolding (thinking block, tool calls, workflow groups)
recedes to **metadata**; the model's answer and the rendered data are the
content. Status reads as a quiet tag, not an alert.

## Decisions (locked with user)

- **daisyUI:** borrow the look only. Keep Tailwind v4 + bits-ui. No plugin.
- **Scope:** chat presentation + two visible chart bugs. No sidebar, dashboard,
  settings, finance sub-views, or onboarding.
- **Badges:** soft tonal (tinted background + colored text), not solid fills,
  not dot-only.

## Files touched (5)

1. `src/app.css` — add soft tonal tokens.
2. `src/lib/components/segments/ToolCallSegment.svelte` — soft badges, receding chrome.
3. `src/lib/components/segments/WorkflowGroup.svelte` — soft badge, receding chrome.
4. `src/lib/components/segments/ThinkingBlock.svelte` — receding chrome.
5. `src/lib/blocks/Chart.svelte` — y-axis formatter + donut legend fix.
6. `src/lib/components/Composer.svelte` — chip styling consistency (no behavior change).

(6 files; #6 is a small consistency follow-on.)

## 1. Token additions — `src/app.css`

Add three tonal surface pairs (tinted bg + readable foreground) to **both**
`:root` and `.dark`, derived from the existing `--success` / `--warning` /
`--destructive` hues. These are the daisyUI-badge / Claude-chip pattern.

`:root` (light) — low-chroma tints on near-white:

```css
--success-soft: oklch(0.95 0.04 150);
--success-soft-foreground: oklch(0.45 0.11 150);
--warning-soft: oklch(0.95 0.05 75);
--warning-soft-foreground: oklch(0.48 0.12 70);
--destructive-soft: oklch(0.95 0.04 27);
--destructive-soft-foreground: oklch(0.51 0.19 27);
```

`.dark` — tints on the slate card surface, brighter foreground:

```css
--success-soft: oklch(0.32 0.05 150);
--success-soft-foreground: oklch(0.82 0.13 150);
--warning-soft: oklch(0.34 0.06 75);
--warning-soft-foreground: oklch(0.85 0.13 80);
--destructive-soft: oklch(0.33 0.06 27);
--destructive-soft-foreground: oklch(0.82 0.15 27);
```

Wire each into `@theme inline`:

```css
--color-success-soft: var(--success-soft);
--color-success-soft-foreground: var(--success-soft-foreground);
--color-warning-soft: var(--warning-soft);
--color-warning-soft-foreground: var(--warning-soft-foreground);
--color-destructive-soft: var(--destructive-soft);
--color-destructive-soft-foreground: var(--destructive-soft-foreground);
```

These exact oklch values are a starting point; verify contrast visually in both
themes during the screenshot pass and nudge lightness if a badge is hard to read.

## 2. Status badges → soft tonal

`running` is the neutral/in-progress state. Map:

- `running` → `bg-warning-soft text-warning-soft-foreground` (amber-tinted, "in flight")
- `done` / `ok` → `bg-success-soft text-success-soft-foreground`
- `error` → `bg-destructive-soft text-destructive-soft-foreground`

Keep the pill shape (`rounded-full px-2 py-0.5`), the icons (`Loader`/`CheckCircle`/
`XCircle` and the spin animation), and the `font-display`/`font-mono` choices
exactly as they are. **Only the bg/text color classes change.**

Call sites:

- `ToolCallSegment.svelte` — the three `{#if status === ...}` pills (lines ~59–74).
  Note current `running` uses `bg-muted text-muted-foreground`; change to warning-soft.
- `WorkflowGroup.svelte` — the three pills in the `pill === ...` block (lines ~34–40).
  Current `running` is `bg-muted`; change to warning-soft. `done`/`error` to soft pairs.

## 3. Card chrome recedes

Three components share the wrapper `class="rounded-md border border-border bg-card …"`.
Toward Claude's quiet collapsed rows:

- **Collapsed wrapper:** drop `bg-card`; soften the border to `border-border/60`.
  The row is near-invisible until interacted with.
- **Header button:** add `hover:bg-muted/40` (and keep existing focus ring) so it
  reads as interactive on hover.
- When **expanded**, the existing `border-t border-border` dividers stay (they're
  what gives an open card its structure). Optionally restore a faint `bg-card` on
  the wrapper only when open, if the open state looks too flat — decide visually.

Apply to `ThinkingBlock.svelte`, `ToolCallSegment.svelte`, `WorkflowGroup.svelte`.

**Invariant:** no change to `aria-expanded`, `disabled`, the chevron rotation, the
`open`/`cardOpen` state, the `{#if open}` bodies, or any prop. Behavior is byte-for-
byte identical; only background/border utility classes change.

## 4. Chart bugs — `Chart.svelte`

### 4a. y-axis number formatter (the `8000.0000000000000` bug)

The non-arc branch (`return { ...base, …, xaxis: … }`) has no `yaxis`, so ApexCharts
prints raw float ticks. Add a `yaxis` with a compact formatter:

```js
yaxis: {
  labels: {
    formatter: (v) => {
      if (!Number.isFinite(v)) return "";
      const abs = Math.abs(v);
      if (abs >= 1000) return `${(v / 1000).toFixed(abs % 1000 === 0 ? 0 : 1)}k`;
      // strip trailing-zero float noise for sub-1000 values
      return Number.isInteger(v) ? String(v) : String(Math.round(v * 100) / 100);
    },
  },
},
```

Result: `8000` → `8k`, `6000` → `6k`, `172.5` → `172.5`, `50` → `50`. No more
floating-point tails.

### 4b. donut legend / pin-icon collision

In the arc branch, the legend sits top-right and collides with the block's pin
overlay (visible in screenshot 1). Move the legend to the bottom (clears the pin
entirely and is the conventional donut layout) and let labels truncate:

```js
if (isArc) {
  return {
    ...base,
    series: (datasets[0]?.data ?? []).map((n) => (Number.isFinite(n) ? n : 0)),
    labels,
    legend: { position: "bottom", horizontalAlign: "center" },
  };
}
```

Add a matching `legend: { position: "bottom" }` consideration for bar/line only if
the same overlap appears there; the screenshots only show it on the donut, so the
arc branch is the required fix. Verify the pin icon no longer overlaps the legend
after the change.

## 5. Composer chip consistency — `Composer.svelte`

No behavior change. These chips are **not status**, so they do not get the §2 soft
tonal colors. The single consistency edit: soften their fill from `bg-muted` to
`bg-muted/60` so they sit quieter against the composer, matching the receding
chrome in §3. Applies to the active-skill chip, attachment chips, and paste chips.
The send button and context bar (`◇ N sources · project`) are unchanged. This is the
lowest-priority change; if the screenshot pass shows no improvement, leave the chips
at `bg-muted`.

## Quality floor

- Verify in **both** light and dark themes (screenshots).
- Every `focus-visible:ring-*` preserved.
- Reduced-motion already handled globally in `app.css` — untouched.
- Responsive layout untouched (only color/spacing utility classes change).
- `cargo build` is not the gate here (frontend only) — run the dev app and
  screenshot the chat turn, an expanded tool call, a workflow group, and a chart
  in both themes.

## Explicitly NOT touched

IPC, stores, session logic, the `items` / `thinkingText` / `stepCount` derivations
in `Message.svelte`, the user-bubble markup, finance views, sidebar, settings
dialog, onboarding, fonts, the agent-spine signature keyframe.

## Verification checklist

- [ ] Tool-call badge: running=amber-soft, done=green-soft, error=red-soft, both themes.
- [ ] Workflow badge soft in all three states.
- [ ] Collapsed thinking/tool/workflow rows are quiet; hover shows interactivity.
- [ ] Expanded rows still legible with dividers.
- [ ] Chart y-axis shows `8k` not `8000.0000000000000`.
- [ ] Donut legend no longer collides with the pin icon.
- [ ] All collapse/expand, copy, focus behaviors work exactly as before.
