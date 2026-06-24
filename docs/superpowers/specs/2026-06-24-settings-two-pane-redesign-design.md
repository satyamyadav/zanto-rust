# Settings dialog — two-pane redesign (design)

_Date: 2026-06-24_

## Goal

Redesign the Settings dialog from a single 80vw scrolling column into a
**two-pane shell** (left grouped nav + right content pane), taking the visual
structure from the "Orbit Cowork" mockups the user supplied. This is a
**presentational reorganization**: every control, every `ipc`/`appStore` call,
and all form-state logic in `SettingsDialog.svelte` is preserved exactly. No
backend, IPC, store, or config-schema change.

Direction is the user's reference mockup: a left sidebar with grouped nav
(MODELS / APP), a right content pane with a per-section **title + description**
header, and an `esc` / Close affordance pinned at the bottom of the sidebar.

## Decisions (locked with user)

1. **Full two-pane shell** — left nav (grouped, with icons) + right content pane
   per section, with a title/description header and an esc hint.
2. **Theme pane:** two **swatch cards** — "Paper" (light) and "Midnight" (dark) —
   each previewing its real palette, with a check on the active one; plus the
   existing density segmented control. **No new themes** are added; the cards map
   to the existing `mode` light/dark. (The mockup's Slate/Solar are NOT built.)
3. **Providers pane:** the `provider_registry` rendered as **selectable cards**
   (letter-avatar, label, endpoint in mono, an "Active" pill on the selected
   provider) — replacing the dropdown. Clicking a card sets it active (same
   `activeProvider` state + `ensureProviderPatch`). The Configure fields
   (model / endpoint or API key / overrides) render below for the selected
   provider, unchanged.

## Nav structure (left pane)

Group **MODELS**
- **Providers** — provider cards + Configure (model, endpoint/key, overrides, Save changes)

Group **APP**
- **Theme** — swatch cards + density (was "Appearance")
- **Folder access** — allowed-paths list + Add folder (unchanged controls)
- **Context** — summarize-beyond-turns (unchanged)
- **Generation** — global GenerationFields (unchanged)
- **Skill** — active-skill select (unchanged)

The current dialog has these exact 7 logical sections; they map 1:1 to nav items
(Provider&model→Providers under MODELS; the other 5 + Theme under APP). Nothing
is added or removed — only regrouped behind nav.

Each nav item: a Lucide icon + label, active item highlighted (accent surface).
Pinned at the sidebar bottom: a "Close" row with an `esc` key hint (closes the
dialog — same as the existing close).

## Right pane (content)

A single scroll container that shows only the selected section. Each section
opens with a header:

- **Title** (`font-display`, ~text-lg)
- **Description** — one muted line explaining the section (new copy, written
  per the writing guidance: plain, end-user framed). Examples:
  - Providers: "Choose where zanto gets its intelligence. API keys are stored in
    your system keychain."
  - Theme: "Pick a color scheme and how dense the layout feels. Changes apply
    instantly."

The body below the header is the **existing controls for that section, moved
verbatim** — same bindings, same handlers, same `Save` buttons.

## Providers pane detail

Replace the `Select.Root` active-provider dropdown with a card list built from
`registry` (`ProviderInfo[]`). For each `r` in `registry`:

```
[Av] {r.label}                         {Active pill if r.id === activeProvider}
     {endpoint shown in mono}
```

- **Avatar:** a rounded square with the provider's first 1–2 letters, on a
  per-provider tint. Derive the tint deterministically from `r.id` (a small hash
  → hue) so it's stable without hardcoding brand colors. Use the existing token
  palette, not new brand hexes.
- **Endpoint line:** `r.default_endpoint ?? "—"` in `font-mono text-xs`.
- **Active pill:** the soft-tonal style from the recent chat-polish work
  (`bg-success-soft text-success-soft-foreground` with a small dot), shown on the
  card whose `r.id === activeProvider`.
- **Click:** `activeProvider = r.id; ensureProviderPatch(r.id)` — identical to the
  current `onValueChange`. Card is a `<button>` with `aria-pressed`,
  `focus-visible:ring`, and the receding-chrome hover (`hover:bg-muted/40`).
- The mockup shows per-provider on/off toggles; the app has **no enable/disable-
  provider concept**, so we do NOT add toggles — selection is the single active
  one (Active pill). Don't invent a backend feature.

Below the card list, the **Configure {label}** block: the existing model input +
Refresh, the endpoint OR api-key block (the `activeInfo.needs_key` branch,
verbatim), and the per-provider generation overrides — all unchanged. Keep the
existing **Save changes** button; in the mockup it (with an optional "Test
connection") sits bottom-right of the configure block — placement only, no new
"Test connection" action is added (YAGNI — there's no IPC for it).

## Theme pane detail

Replace the Light/Dark `<Button>` pair with two **swatch cards** in a row:

- **Paper** (light): preview strip of the light palette (e.g. background, card,
  muted, primary swatches), name "Paper", description "Bright light theme with a
  violet accent.", check icon when `mode.current === "light"`. Click → `setMode("light")`.
- **Midnight** (dark): same, dark palette, "Deep dark theme with a violet accent.",
  active when `mode.current === "dark"`. Click → `setMode("dark")`.

The palette preview is rendered with the actual CSS token values (small colored
divs), so it always matches the real theme. Cards are `<button>`s with
`aria-pressed`, focus ring, accent ring when active.

Keep the existing **density** segmented control below, verbatim. The displayed
labels stay "Compact / Normal / Relaxed" (the mockup's "Compressed" wording is
not adopted — the `Density` value is `compact`, and changing only the visible
label adds confusion for no gain). Density logic unchanged.

## Shell / layout

- `Dialog.Content`: widen to the mockup proportion (`sm:max-w-[860px]`,
  `h-[80vh]`), `flex` row: left nav fixed width (~190px), right pane `flex-1`.
- Left nav: `bg-sidebar` (existing token) or `bg-muted/40`, vertical, group
  headings in `text-[10px] uppercase tracking-wide text-muted-foreground`.
- Right pane: `overflow-y-auto`, padded, the active section only.
- Keep the existing `Dialog.Root bind:open`, the seed-from-config `$effect`, and
  the close behavior. Active-section state is new local `$state` (default
  "providers").

## State additions (the only new logic)

- `let section = $state<SectionId>("providers")` — which nav item is open.
- `type SectionId = "providers" | "theme" | "folders" | "context" | "generation" | "skill"`.
- A `NAV` array describing groups → items (id, label, icon) for rendering the
  sidebar. Pure presentation.
- A deterministic `avatarTint(id: string)` helper (hash → one of N token-based
  tints) for provider avatars.

Everything else — every `$state`, `$derived`, `$effect`, and async handler in the
current file — stays exactly as-is.

## What is explicitly NOT changed

- No IPC / Rust / config-schema change.
- No new themes (Slate/Solar not built).
- No provider enable/disable toggles, no "Test connection" action (no backend).
- No change to GenerationFields, the model combobox, key save/clear flow, folder
  picker, context-turns, or skill select — only their container moves.
- The `density` values and labels unchanged.

## Quality floor

- Keyboard: nav items and cards are focusable, `aria-pressed`/`aria-current`,
  visible focus rings. esc closes (existing Dialog behavior).
- Both themes verified (light + dark) via the mock dev server.
- Responsive: at narrow widths the dialog already caps at viewport; the two-pane
  row should keep the nav usable (min width) — verify nothing clips.
- Reduced-motion respected (no new animation beyond existing).
- `pnpm check` clean; `pnpm test:ui` green (Settings has no dedicated UI test, but
  the suite must not regress).

## Risk notes

- This is the largest single component in the settings flow; the redesign moves a
  lot of markup. The mitigation: move blocks **verbatim** into section wrappers,
  changing only their surrounding container and the two replaced controls
  (provider dropdown → cards, theme buttons → swatches). Diff should read as
  "wrap + 2 control swaps", not a rewrite of the logic.
- The seed `$effect` reads `open`/`appStore.config` and writes form state under
  `untrack` — must remain untouched so provider selection doesn't clobber.

## Verification checklist

- [ ] Two-pane shell renders: grouped nav left, section content right, esc hint.
- [ ] Each of the 6 nav items shows its section; switching preserves form state.
- [ ] Providers: cards from registry, Active pill on the selected, click selects +
      ensures patch; Configure block works (model/endpoint/key/overrides/Save).
- [ ] Theme: Paper/Midnight swatch cards reflect + set mode; density works.
- [ ] Folder access, Context, Generation, Skill behave exactly as before.
- [ ] Both themes legible; focus rings present; esc closes.
- [ ] `pnpm check` 0/0; `pnpm test:ui` green.
