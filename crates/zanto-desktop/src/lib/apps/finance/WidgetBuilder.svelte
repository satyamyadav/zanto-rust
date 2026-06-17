<script lang="ts">
  import { ipc } from "$lib/ipc";
  import { ChevronUp, ChevronDown, Trash2, Plus, Check } from "@lucide/svelte";

  export type WidgetKind = "kpi" | "chart" | "table";
  export type Widget = { kind: WidgetKind; title: string; source: string };

  // F4 — lightweight dashboard editor. Add/remove/reorder widgets, then persist
  // via `save_widgets`. `onSaved` lets the dashboard re-load the layout.
  let { widgets = $bindable<Widget[]>([]), onSaved }: {
    widgets: Widget[];
    onSaved?: () => void;
  } = $props();

  // `source` options keyed by widget kind — each names a slice of `overview`.
  const SOURCES: Record<WidgetKind, { value: string; label: string }[]> = {
    kpi: [
      { value: "balance", label: "Balance" },
      { value: "month_total", label: "This month's spend" },
      { value: "transaction_count", label: "Transaction count" },
    ],
    chart: [{ value: "series", label: "6-month spend series" }],
    table: [{ value: "top_categories", label: "Top categories" }],
  };

  let saving = $state(false);
  let error = $state<string | null>(null);

  function add(kind: WidgetKind) {
    const src = SOURCES[kind][0];
    widgets = [...widgets, { kind, title: src.label, source: src.value }];
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

<div class="space-y-3 rounded-lg border border-border p-3">
  <div class="text-sm font-medium">Edit dashboard</div>

  {#if widgets.length === 0}
    <div class="text-sm text-muted-foreground">No widgets. Add one below.</div>
  {/if}

  <ul class="space-y-2">
    {#each widgets as w, i (i)}
      <li class="flex items-center gap-2 rounded-md border border-border p-2">
        <div class="flex flex-col">
          <button
            type="button"
            class="text-muted-foreground hover:text-foreground disabled:opacity-30"
            onclick={() => move(i, -1)}
            disabled={i === 0}
            aria-label="Move up"
          >
            <ChevronUp class="size-4" />
          </button>
          <button
            type="button"
            class="text-muted-foreground hover:text-foreground disabled:opacity-30"
            onclick={() => move(i, 1)}
            disabled={i === widgets.length - 1}
            aria-label="Move down"
          >
            <ChevronDown class="size-4" />
          </button>
        </div>

        <select
          class="rounded-md border border-border bg-background px-2 py-1 text-xs outline-none focus:ring-1 focus:ring-ring"
          value={w.kind}
          onchange={(e) => setKind(i, e.currentTarget.value as WidgetKind)}
        >
          <option value="kpi">KPI</option>
          <option value="chart">Chart</option>
          <option value="table">Table</option>
        </select>

        <select
          class="rounded-md border border-border bg-background px-2 py-1 text-xs outline-none focus:ring-1 focus:ring-ring"
          bind:value={w.source}
        >
          {#each SOURCES[w.kind] as s (s.value)}
            <option value={s.value}>{s.label}</option>
          {/each}
        </select>

        <input
          class="min-w-0 flex-1 rounded-md border border-border bg-background px-2 py-1 text-xs outline-none focus:ring-1 focus:ring-ring"
          bind:value={w.title}
          placeholder="Title"
        />

        <button
          type="button"
          class="text-muted-foreground hover:text-destructive"
          onclick={() => remove(i)}
          aria-label="Remove widget"
        >
          <Trash2 class="size-4" />
        </button>
      </li>
    {/each}
  </ul>

  {#if error}
    <div class="text-xs text-destructive">Couldn't save: {error}</div>
  {/if}

  <div class="flex flex-wrap items-center gap-2">
    <button
      type="button"
      class="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:bg-muted"
      onclick={() => add("kpi")}
    >
      <Plus class="size-3.5" /> KPI
    </button>
    <button
      type="button"
      class="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:bg-muted"
      onclick={() => add("chart")}
    >
      <Plus class="size-3.5" /> Chart
    </button>
    <button
      type="button"
      class="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-xs hover:bg-muted"
      onclick={() => add("table")}
    >
      <Plus class="size-3.5" /> Table
    </button>
    <button
      type="button"
      class="ml-auto inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
      onclick={save}
      disabled={saving}
    >
      <Check class="size-3.5" />
      {saving ? "Saving…" : "Save layout"}
    </button>
  </div>
</div>
