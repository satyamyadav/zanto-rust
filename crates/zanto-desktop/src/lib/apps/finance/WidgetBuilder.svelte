<script lang="ts">
  import { ipc } from "$lib/ipc";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { GripVertical, ChevronUp, ChevronDown, Copy, Trash2, Plus, Check } from "@lucide/svelte";

  export type WidgetKind = "kpi" | "chart" | "table";
  export type Widget = { kind: WidgetKind; title: string; source: string };

  // F4 — lightweight dashboard editor. Add/remove/reorder/duplicate widgets, then
  // persist via `save_widgets`. `onSaved` lets the dashboard re-load the layout.
  let { widgets = $bindable<Widget[]>([]), onSaved }: {
    widgets: Widget[];
    onSaved?: () => void;
  } = $props();

  // `source` options keyed by widget kind — each names a slice of `overview`.
  const SOURCES: Record<WidgetKind, { value: string; label: string }[]> = {
    kpi: [
      { value: "balance", label: "Balance" },
      { value: "month_total", label: "This month's spend" },
      { value: "income", label: "Income (this month)" },
      { value: "net_cash_flow", label: "Net cash flow" },
      { value: "transaction_count", label: "Transaction count" },
    ],
    chart: [{ value: "series", label: "6-month spend series" }],
    table: [{ value: "top_categories", label: "Top categories" }],
  };

  let saving = $state(false);
  let error = $state<string | null>(null);

  // Shared select styling — token-based, with a visible focus ring.
  const selectClass =
    "h-7 rounded-md border border-border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring";

  function add(kind: WidgetKind) {
    const src = SOURCES[kind][0];
    widgets = [...widgets, { kind, title: src.label, source: src.value }];
  }

  function duplicate(i: number) {
    const next = [...widgets];
    next.splice(i + 1, 0, { ...widgets[i] });
    widgets = next;
  }

  function remove(i: number) {
    widgets = widgets.filter((_, j) => j !== i);
  }

  function move(i: number, delta: number) {
    const j = i + delta;
    if (j < 0 || j >= widgets.length) return;
    const next = [...widgets];
    [next[i], next[j]] = [next[j], next[i]];
    widgets = next;
  }

  function setKind(i: number, kind: WidgetKind) {
    const src = SOURCES[kind][0];
    const next = [...widgets];
    next[i] = { ...next[i], kind, source: src.value };
    widgets = next;
  }

  async function save() {
    saving = true;
    error = null;
    try {
      await ipc.runAppAction("finance", "save_widgets", { widgets });
      onSaved?.();
    } catch (e) {
      error = `${e}`;
    } finally {
      saving = false;
    }
  }
</script>

<div class="space-y-3 rounded-lg border border-border bg-card p-3">
  <div class="font-display text-sm font-semibold">Edit dashboard</div>

  {#if widgets.length === 0}
    <div class="rounded-md border border-dashed border-border p-3 text-sm text-muted-foreground">
      No widgets yet. Add a KPI, chart, or table below.
    </div>
  {/if}

  <ul class="space-y-2">
    {#each widgets as w, i (i)}
      <li class="flex items-center gap-2 rounded-md border border-border p-2">
        <div class="flex items-center gap-1 text-muted-foreground" aria-hidden="true">
          <GripVertical class="size-4" />
        </div>

        <div class="flex flex-col">
          <button
            type="button"
            class="rounded-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30"
            onclick={() => move(i, -1)}
            disabled={i === 0}
            aria-label="Move widget up"
          >
            <ChevronUp class="size-4" />
          </button>
          <button
            type="button"
            class="rounded-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30"
            onclick={() => move(i, 1)}
            disabled={i === widgets.length - 1}
            aria-label="Move widget down"
          >
            <ChevronDown class="size-4" />
          </button>
        </div>

        <select
          class={selectClass}
          value={w.kind}
          onchange={(e) => setKind(i, e.currentTarget.value as WidgetKind)}
          aria-label="Widget type"
        >
          <option value="kpi">KPI</option>
          <option value="chart">Chart</option>
          <option value="table">Table</option>
        </select>

        <select class={selectClass} bind:value={w.source} aria-label="Data source">
          {#each SOURCES[w.kind] as s (s.value)}
            <option value={s.value}>{s.label}</option>
          {/each}
        </select>

        <Input class="h-7 min-w-0 flex-1 text-xs" bind:value={w.title} placeholder="Title" />

        <button
          type="button"
          class="rounded-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
          onclick={() => duplicate(i)}
          aria-label="Duplicate widget"
        >
          <Copy class="size-4" />
        </button>
        <button
          type="button"
          class="rounded-sm text-muted-foreground outline-none hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring"
          onclick={() => remove(i)}
          aria-label="Remove widget"
        >
          <Trash2 class="size-4" />
        </button>
      </li>
    {/each}
  </ul>

  {#if error}
    <div class="text-xs text-destructive">Couldn't save the layout: {error}. Try again.</div>
  {/if}

  <div class="flex flex-wrap items-center gap-2">
    <Button variant="outline" size="xs" onclick={() => add("kpi")}>
      <Plus /> KPI
    </Button>
    <Button variant="outline" size="xs" onclick={() => add("chart")}>
      <Plus /> Chart
    </Button>
    <Button variant="outline" size="xs" onclick={() => add("table")}>
      <Plus /> Table
    </Button>
    <Button class="ml-auto" size="sm" onclick={save} disabled={saving}>
      <Check />
      {saving ? "Saving…" : "Save layout"}
    </Button>
  </div>
</div>
