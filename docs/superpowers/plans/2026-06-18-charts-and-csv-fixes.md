# Charts + CSV Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make charts actually render (dedicated one-shot `chart` tool + ApexCharts SVG renderer), stop showing artifact-rendering tool calls as tool cards, and clear the remaining actionable CSV findings.

**Architecture:** The chart failure (CSV A-2, SS-3) is a *tool-protocol* problem, not a renderer problem: the small local model never completes the `list_artifacts → get_artifact → render_artifact(schema)` dance and falls back to base64 PNGs / markdown tables. Fix it by adding a dedicated, lenient `chart` tool the model calls in **one step** with flat input — it returns a normal component `Block` (same path as `render_artifact`), so it renders inline like any artifact. Separately, artifact-rendering tool calls (`chart`, `render_artifact`, `list_artifacts`, `get_artifact`, `pin_artifact`) are *internal plumbing*; hide their tool-call cards at render time so the produced block reads like markdown/text. The chart block itself is re-implemented on **ApexCharts** (SVG-native, no canvas → WebKitGTK-safe). The rest are small, localized UI fixes.

**Tech Stack:** Rust (zanto-core, zanto-desktop src-tauri, genai tools, jsonschema), Svelte 5 runes + Tailwind v4, ApexCharts (SVG-native), pnpm.

**Decisions (locked with user):**
- Chart library: **ApexCharts** (SVG-native, WebKitGTK-safe). Replace the pure-SVG `Chart.svelte`.
- Add a dedicated `chart` tool; keep `render_artifact` for other artifacts.
- Hide artifact-rendering tool-call cards from the chat UI (frontend-only, render-time).
- `/clear` → always available, clears the composer (input + pastes + attachments). Thread-clear stays out of scope (`/new` covers it).

**Verify gate (run after every task that changes Rust or frontend):**
```bash
cargo build
cargo test -p zanto-core
cargo test -p zanto-desktop            # catalogue.rs unit tests live in the desktop crate
export PATH="$HOME/.nvm/versions/node/v24.16.0/bin:$PATH"
cd crates/zanto-desktop && pnpm check && pnpm build:web
```
A green build is the compile gate only. The user smoke-tests `pnpm dev` at the review gate.

---

### Task 1: Dedicated one-shot `chart` tool (backend)

**Files:**
- Modify: `crates/zanto-desktop/src-tauri/src/catalogue.rs` (add `chart` to `shared_tools()`, add `chart_data_from_args` helper, handle `"chart"` in `dispatch`, add unit tests)
- Modify: `crates/zanto-desktop/src-tauri/src/ipc/chat.rs:41-63` (`ARTIFACT_PROTOCOL` — point the model at `chart`)

- [ ] **Step 1: Write the failing tests** in `catalogue.rs` (append inside the existing `mod tests`):

```rust
    #[test]
    fn chart_tool_normalizes_values_shortcut() {
        // The lenient `chart` tool accepts a single-series `values` array and
        // wraps it into the catalogue chart's `datasets` shape.
        let args = json!({ "type": "bar", "labels": ["Mon", "Tue"], "values": [120, 200], "title": "Weekly" });
        let data = chart_data_from_args(&args);
        assert_eq!(data["datasets"][0]["data"], json!([120, 200]));
        assert_eq!(data["title"], json!("Weekly"));
        Catalogue::load().validate("chart", &data).expect("normalized chart must validate");
    }

    #[test]
    fn chart_tool_accepts_explicit_datasets() {
        let args = json!({ "type": "line", "labels": ["a", "b"], "datasets": [{ "label": "x", "data": [3, 4] }] });
        let data = chart_data_from_args(&args);
        assert_eq!(data["datasets"][0]["label"], json!("x"));
        Catalogue::load().validate("chart", &data).expect("explicit datasets chart must validate");
    }
```

- [ ] **Step 2: Run the tests — expect FAIL** (`chart_data_from_args` undefined):

Run: `cargo test -p zanto-desktop chart_tool`
Expected: FAIL — `cannot find function chart_data_from_args`.

- [ ] **Step 3: Add the `chart_data_from_args` helper** (place it above `shared_tools()` in `catalogue.rs`):

```rust
/// Build the chart component's `data` object from the lenient `chart` tool args.
/// Accepts either explicit `datasets` or a single-series `values` shortcut, so a
/// weak model can render a chart in one call without learning the dataset shape.
pub fn chart_data_from_args(args: &Value) -> Value {
    let chart_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("bar");
    let labels = args.get("labels").cloned().unwrap_or_else(|| json!([]));
    let datasets = match args.get("datasets") {
        Some(ds) if ds.as_array().map(|a| !a.is_empty()).unwrap_or(false) => ds.clone(),
        _ => json!([{ "data": args.get("values").cloned().unwrap_or_else(|| json!([])) }]),
    };
    let mut data = json!({ "type": chart_type, "labels": labels, "datasets": datasets });
    if let Some(t) = args.get("title") {
        data["title"] = t.clone();
    }
    data
}
```

- [ ] **Step 4: Register the `chart` tool** — append this `GenaiTool` to the `vec![ ... ]` in `shared_tools()` (after `render_artifact`, before `pin_artifact`):

```rust
        GenaiTool::new("chart")
            .with_description("Show a chart to the user, inline in the chat. Call this DIRECTLY with the data — do not call list_artifacts or get_artifact first, and do not announce a chart without calling this. For one series pass `values`; for multiple series pass `datasets`. `labels` are the category names (x-axis or slices).")
            .with_schema(json!({
                "type": "object",
                "properties": {
                    "type": { "type": "string", "enum": ["bar", "line", "pie", "doughnut"] },
                    "title": { "type": "string" },
                    "labels": { "type": "array", "items": { "type": "string" } },
                    "values": { "type": "array", "items": { "type": "number" }, "description": "Single-series shortcut. Use this OR datasets, not both." },
                    "datasets": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "label": { "type": "string" },
                                "data": { "type": "array", "items": { "type": "number" } }
                            },
                            "required": ["data"]
                        }
                    }
                },
                "required": ["type", "labels"]
            })),
```

- [ ] **Step 5: Handle `"chart"` in `dispatch`** — add this arm in `SharedDispatcher::dispatch`, immediately before `_ => self.app.dispatch_tool(...)`:

```rust
            "chart" => {
                let data = chart_data_from_args(&args);
                match self.catalogue.validate("chart", &data) {
                    Ok(()) => Some(Ok(AppResult::Block {
                        component_id: "chart".to_string(),
                        data,
                        target: Target::Inline,
                    })),
                    Err(details) => Some(Ok(AppResult::Data(json!({
                        "error": "chart data was invalid. Provide `type`, `labels`, and either `values` (one series) or `datasets`, then call chart again.",
                        "details": details,
                    })))),
                }
            }
```

- [ ] **Step 6: Run the tests — expect PASS:**

Run: `cargo test -p zanto-desktop chart_tool`
Expected: PASS (2 tests).

- [ ] **Step 7: Point the model at `chart`** in `ipc/chat.rs` — insert this sentence into `ARTIFACT_PROTOCOL` right after the first sentence (after "...shows the user nothing."), so charts skip the discovery dance:

```rust
To show a chart specifically, call `chart({type, labels, values})` directly in ONE step — \
do NOT use list_artifacts/get_artifact/render_artifact for charts. \
```

- [ ] **Step 8: Verify gate** (cargo build + tests + pnpm check/build:web). All clean.

- [ ] **Step 9: Commit**

```bash
git add crates/zanto-desktop/src-tauri/src/catalogue.rs crates/zanto-desktop/src-tauri/src/ipc/chat.rs
git commit -m "feat(chart): dedicated one-shot chart tool (lenient values/datasets), inline block"
```

---

### Task 2: Hide artifact-rendering tool-call cards (frontend)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/Message.svelte` (filter hidden tool calls out of `items`; exclude them from `stepCount`)

Rationale: artifact tools are internal plumbing — `chart`/`render_artifact` already produce a `block` segment that renders inline, and `list_artifacts`/`get_artifact`/`pin_artifact` produce nothing the user needs to see. Drop their cards at render time. This is purely presentational; the segments stay in the persisted data model, so the behaviour is identical live and on session reopen.

- [ ] **Step 1: Add the hidden-tool set** — after the `ToolCallSegmentData` type alias (around line 18) in `Message.svelte`:

```ts
  // Artifact-system tool calls are internal: their result renders inline as a
  // block (chart/render_artifact) or is pure plumbing (list/get/pin). We never
  // show a tool-call card for them — the artifact reads like markdown/text.
  const HIDDEN_TOOL_CALLS = new Set([
    "chart",
    "render_artifact",
    "list_artifacts",
    "get_artifact",
    "pin_artifact",
  ]);
```

- [ ] **Step 2: Drop hidden tool calls from inline `items`** — in the `items` derived (`const segs = entry.segments.filter(...)`), add a clause so the filter reads:

```ts
    const segs = entry.segments.filter((seg, idx) => {
      if (seg.kind === "reasoning") return false;
      if (seg.kind === "text" && idx < lti) return false;
      if (seg.kind === "tool_call" && HIDDEN_TOOL_CALLS.has(seg.name)) return false;
      return true;
    });
```

- [ ] **Step 3: Exclude hidden calls from the thinking step count** — change the `stepCount` derived so a chart/render turn isn't summarised as "Thought for N steps" for invisible plumbing:

```ts
  const stepCount = $derived(
    entry.segments.filter(
      (s) => s.kind === "tool_call" && !HIDDEN_TOOL_CALLS.has(s.name),
    ).length,
  );
```

- [ ] **Step 4: Verify gate** (`pnpm check && pnpm build:web`). Expect 0 errors.

- [ ] **Step 5: Manual check note (for the review gate):** Ask for a chart → the chart block renders inline with **no** `render_artifact`/`chart` tool card above it, and no "Thought for N steps" attributable solely to rendering. A genuine tool turn (e.g. `read_file`) still shows its card.

- [ ] **Step 6: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/Message.svelte
git commit -m "feat(chat): render artifact tool calls inline as blocks, hide their tool cards"
```

---

### Task 3: ApexCharts chart block (frontend)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/blocks/Chart.svelte` (replace pure-SVG impl with an ApexCharts wrapper)
- Modify: `crates/zanto-desktop/package.json` (add `apexcharts`)

Notes: ApexCharts renders SVG natively (no canvas) → WebKitGTK-safe. It is imported **dynamically inside `onMount`** so `pnpm build:web` (prerender) never touches `window` at build time. Series colors use a fixed hex palette (not `oklch` CSS vars) to avoid any webview color-space gaps in SVG fills. `doughnut` maps to ApexCharts `donut`.

- [ ] **Step 1: Add the dependency**

Run:
```bash
export PATH="$HOME/.nvm/versions/node/v24.16.0/bin:$PATH"
cd crates/zanto-desktop && pnpm add apexcharts
```
Expected: `apexcharts` added to `package.json` dependencies; lockfile updated.

- [ ] **Step 2: Replace `Chart.svelte`** with the ApexCharts wrapper (full file):

```svelte
<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  type ChartData = {
    type: "bar" | "line" | "pie" | "doughnut";
    title?: string;
    labels: string[];
    datasets: { label?: string; data: number[] }[];
  };

  let { data }: { data: ChartData } = $props();

  // Fixed hex palette (violet-led, brand-aligned) — deliberately NOT oklch CSS
  // vars, so SVG fills are safe across webview color-space support.
  const SERIES = ["#7c3aed", "#0ea5e9", "#22c55e", "#f59e0b", "#ef4444", "#a855f7"];
  const LIGHT = { fore: "#52525b", grid: "#e4e4e7" };
  const DARK = { fore: "#a1a1aa", grid: "#27272a" };

  let el: HTMLDivElement;
  let chart: { render: () => Promise<void>; updateOptions: (o: unknown, r?: boolean, a?: boolean) => void; destroy: () => void } | null = null;

  function buildOptions(d: ChartData) {
    const dark = typeof document !== "undefined" && document.documentElement.classList.contains("dark");
    const c = dark ? DARK : LIGHT;
    const isArc = d.type === "pie" || d.type === "doughnut";
    const apexType = d.type === "doughnut" ? "donut" : d.type;
    const labels = d.labels ?? [];
    const datasets = d.datasets ?? [];

    const base: Record<string, unknown> = {
      chart: { type: apexType, height: 256, background: "transparent", toolbar: { show: false }, fontFamily: "inherit", foreColor: c.fore },
      colors: SERIES,
      theme: { mode: dark ? "dark" : "light" },
      tooltip: { theme: dark ? "dark" : "light" },
      grid: { borderColor: c.grid },
      dataLabels: { enabled: false },
      title: d.title ? { text: d.title, style: { fontSize: "12px", fontWeight: 500, color: c.fore } } : undefined,
      noData: { text: "No data", style: { color: c.fore } },
    };

    if (isArc) {
      return { ...base, series: (datasets[0]?.data ?? []).map((n) => (Number.isFinite(n) ? n : 0)), labels };
    }
    return {
      ...base,
      series: datasets.map((ds, i) => ({ name: ds.label ?? `Series ${i + 1}`, data: ds.data ?? [] })),
      xaxis: { categories: labels },
    };
  }

  onMount(async () => {
    const mod = await import("apexcharts");
    const ApexCharts = mod.default;
    chart = new ApexCharts(el, buildOptions(data));
    await chart.render();
  });

  // Re-render in place when the artifact data changes (e.g. streaming update).
  $effect(() => {
    const opts = buildOptions(data);
    if (chart) chart.updateOptions(opts, true, true);
  });

  onDestroy(() => chart?.destroy());
</script>

<div class="w-full" bind:this={el}></div>
```

- [ ] **Step 3: Verify gate** — `pnpm check && pnpm build:web` (confirms the dynamic import keeps prerender clean), then `cargo build`.

- [ ] **Step 4: Manual check note (review gate):** "bar chart of Mon–Sun values 120,200,150,80,70,110,130" draws a real ApexCharts bar chart inline; line/pie/doughnut all draw; dark/light themes both legible; "open in panel" still mounts it in the canvas.

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src/lib/blocks/Chart.svelte crates/zanto-desktop/package.json crates/zanto-desktop/pnpm-lock.yaml
git commit -m "feat(chart): render chart block with ApexCharts (SVG, WebKitGTK-safe)"
```

---

### Task 4: `/clear` always available (CSV C-9)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/Composer.svelte:172-199`

Bug: `/clear` is gated behind `hasClearable`, so typing just `/clear` strips its own fragment → `hasClearable` is false → the command vanishes from the menu ("deadlock"). Fix: list `/clear` unconditionally; `clearInput()` is a harmless no-op when there's nothing to clear.

- [ ] **Step 1: Make `/clear` unconditional** — replace the `SLASH_COMMANDS` derived (and drop the now-unused `hasClearable`):

```ts
  const SLASH_COMMANDS = $derived<SlashCommand[]>([
    { name: "new", hint: "Start a new session", run: () => newSession() },
    { name: "clear", hint: "Clear the composer", run: clearInput },
  ]);
```

- [ ] **Step 2: Remove the dead `hasClearable` derived** (lines ~173-177) so `pnpm check` stays warning-clean.

- [ ] **Step 3: Verify gate** — `pnpm check && pnpm build:web`. Expect 0 errors, no new warnings.

- [ ] **Step 4: Manual check note:** typing `/clear` (with or without other text) and selecting it empties the composer; selecting it on an empty composer is a silent no-op (no deadlock).

- [ ] **Step 5: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/Composer.svelte
git commit -m "fix(composer): /clear always selectable; clears the composer"
```

---

### Task 5: Pin discoverability (CSV A-5)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/Block.svelte:53-66`

Bug: the Pin button is `opacity-0` until hover, so users "cannot find" it (compounded by charts never rendering, so a pinnable view never appeared — Task 1/3 fixes that). Make the affordance faintly visible at rest and fully on hover/focus.

- [ ] **Step 1: Make the Pin button visible at rest** — change its class so it is not fully transparent: replace `opacity-0` with `opacity-60` and keep the hover/focus reveal. Concretely, in the Pin `<button>` class string, swap:

```
opacity-0 backdrop-blur transition-opacity hover:text-foreground focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring group-hover:opacity-100
```
to:
```
opacity-60 backdrop-blur transition-opacity hover:text-foreground hover:opacity-100 focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-ring group-hover:opacity-100
```

- [ ] **Step 2: Verify gate** — `pnpm check && pnpm build:web`.

- [ ] **Step 3: Manual check note:** a rendered chart/table shows a faint pin affordance top-right at rest; pinning then reappears under Artifacts → Pinned views.

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/Block.svelte
git commit -m "fix(artifacts): make Pin affordance visible at rest (A-5 discoverability)"
```

---

### Task 6: App-switch loading state (CSV SS-4)

**Files:**
- Modify: `crates/zanto-desktop/src/lib/components/Sidebar.svelte` (add a spinner while `switching`)

Bug: switching Chat↔Finance only dims the list (`opacity-50`); with no spinner it reads as a lag/hang. Add an explicit loading row while `switching` is true. (The generation race-guard already prevents stale appends; infinite scroll via `loadMoreSessions` already exists — no logic change needed there.)

- [ ] **Step 1: Add a spinner row** at the top of the session-list scroll container (the `div` with `transition-opacity {switching ? ...}` around Sidebar.svelte:165). Import a spinner icon (`import LoaderIcon from "@lucide/svelte/icons/loader";`) if not already imported, and render:

```svelte
      {#if switching}
        <div class="flex items-center justify-center gap-2 py-3 text-xs text-muted-foreground">
          <LoaderIcon class="size-3.5 animate-spin" />
          Switching…
        </div>
      {/if}
```

- [ ] **Step 2: Verify gate** — `pnpm check && pnpm build:web`.

- [ ] **Step 3: Manual check note:** switching apps shows a brief "Switching…" spinner instead of a silent dim; rapid switches don't append threads to the wrong app.

- [ ] **Step 4: Commit**

```bash
git add crates/zanto-desktop/src/lib/components/Sidebar.svelte
git commit -m "fix(sidebar): show loading spinner during app switch (SS-4)"
```

---

### Task 7: Verification pass for likely-stale CSV rows + final review

These rows were marked Fail/Partial in the CSV but the code already implements the requested behaviour (the findings predate the recent fixes). **Do not write speculative fixes.** Smoke-test each on current `main`; only open a fix step if it actually reproduces.

- [ ] **Step 1: C-12 link handling** — confirm `links.svelte.ts:openLinkInPanel` + `Canvas.svelte` promoted-link webview already opens links in the panel with a toolbar (Open in browser / Copy / Close) and never navigates the app. Expected: matches the CSV's requested behaviour. If a real gap remains, note it and fix minimally.

- [ ] **Step 2: A-4 artifact browser** — confirm `panelMode === "browser"` renders `ArtifactBrowser` in the side panel (not a modal) via Sidebar's labeled "Artifacts" button. If overflow/width is genuinely broken at the live size, tighten the `ArtifactBrowser.svelte` flex/overflow; otherwise leave it.

- [ ] **Step 3: C-2 / C-4 persistence** — reopen an old session that had a thinking block, tool calls, and a "Stopped" marker. The persistence round-trip (chat.rs `assistant_turn_meta` → metadata → `toEntries`) restores `reasoning`/`tool_call`/`block`/`stopped`, so this should now survive reload. Edge to check: a turn stopped with **zero** produced segments — `assistant_turn_meta` returns `None` and `display_messages_meta` drops an empty message, so a content-less stopped turn shows no marker. If the user considers that a bug, the fix is to persist a minimal `{segments:[], stopped:true}` for stopped turns; otherwise leave as-is and note it.

- [ ] **Step 4: Final whole-branch verify** — run the full verify gate once more from a clean tree:

```bash
git checkout -- . 2>/dev/null || true
cargo build && cargo test -p zanto-core && cargo test -p zanto-desktop
export PATH="$HOME/.nvm/versions/node/v24.16.0/bin:$PATH"
cd crates/zanto-desktop && pnpm install && pnpm check && pnpm build:web
```
Expected: all green.

- [ ] **Step 5: Update the CSV** — mark A-2, SS-3, C-9, A-5, SS-4 (and any verified-fixed C-2/C-4/C-12/A-4) with their new status + a one-line note referencing this plan. Commit:

```bash
git add docs/zanto-test-checklist.csv
git commit -m "docs: update test checklist after charts + CSV fixes"
```

---

## Out of scope (flagged, not dropped)
- **C-7** pasted-content sub-view inside the user bubble (expandable/editable) — user deferred to P2.
- **H-2** agent ask-form multi-step + full keyboard support — nice-to-have, deferred.
- **DOC-4 / F-1..3 / CO-1..2 / FLOW-*** — untested CSV rows; the user tests these, not this plan.
- Dedicated one-shot tools for `table`/`metric`/etc. (same pattern as the `chart` tool) — only `chart` is in scope now; the others still go through `render_artifact`. Worth doing later if weak models also thrash on tables.

## Self-Review
- **Spec coverage:** charts render (Task 1 tool + Task 3 ApexCharts) ✓; chart tool takes input + renders as a normal block (Task 1) ✓; artifact rendering not shown as a tool call (Task 2) ✓; CSV C-9 ✓ (Task 4), A-5 ✓ (Task 5), SS-4 ✓ (Task 6), A-2/SS-3 ✓ (Tasks 1+3), C-2/C-4/C-12/A-4 verified (Task 7). C-7/H-2 explicitly deferred.
- **Type consistency:** `chart_data_from_args` returns the exact `{type, labels, datasets, title?}` shape the `chart` catalogue `data_schema` requires and that `Chart.svelte`'s `ChatData` consumes; `HIDDEN_TOOL_CALLS` names match the genai tool names in `shared_tools()` (`chart`, `render_artifact`, `list_artifacts`, `get_artifact`, `pin_artifact`); `AppResult::Block { component_id, data, target }` matches the existing `render_artifact` return.
- **Placeholders:** none — every code step carries full code.
