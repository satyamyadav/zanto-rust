<script lang="ts">
  import { ipc } from "$lib/ipc";
  import { Button } from "$lib/components/ui/button";
  import Upload from "@lucide/svelte/icons/upload";
  import CheckCircle2 from "@lucide/svelte/icons/check-circle-2";
  import Info from "@lucide/svelte/icons/info";

  type SampleRow = { date: string; description: string; amount: number };

  // Mock parsed statement preview.
  const SAMPLE: SampleRow[] = [
    { date: "2026-06-21", description: "WHOLE FOODS MARKET #882", amount: -64.12 },
    { date: "2026-06-22", description: "UBER TRIP HELP.UBER.COM", amount: -19.5 },
    { date: "2026-06-23", description: "STARBUCKS STORE 119", amount: -7.4 },
    { date: "2026-06-24", description: "SHELL OIL 23981", amount: -48.0 },
    { date: "2026-06-25", description: "SALARY ACME CORP", amount: 3200.0 },
  ];

  // Stage of the mock flow: choose → preview → done.
  let stage = $state<"choose" | "preview" | "done">("choose");
  let importing = $state(false);

  // Mock column mapping (pre-filled, editable for the demo).
  let mapDate = $state("Column 1 (date)");
  let mapMerchant = $state("Column 2 (description)");
  let mapAmount = $state("Column 3 (amount)");

  const COLS = ["Column 1 (date)", "Column 2 (description)", "Column 3 (amount)"];

  function useSample() {
    stage = "preview";
  }

  async function doImport() {
    importing = true;
    try {
      // Mock: actually push a couple of rows so the rest of the app reflects it.
      for (const r of SAMPLE.slice(0, 2)) {
        await ipc.runAppAction("finance", "add_transaction", {
          merchant: r.description,
          amount: Math.abs(r.amount),
          type: r.amount >= 0 ? "income" : "expense",
          date: r.date,
          account: "Checking",
        });
      }
      stage = "done";
    } finally {
      importing = false;
    }
  }

  function reset() {
    stage = "choose";
  }

  const selectClass =
    "h-8 rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring";
</script>

<div class="space-y-4">
  {#if stage === "choose"}
    <div
      class="flex flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed border-border p-10 text-center"
    >
      <div class="rounded-full bg-accent p-3">
        <Upload class="size-6 text-accent-foreground" />
      </div>
      <div class="space-y-1">
        <div class="font-display text-base font-semibold">Import a statement</div>
        <div class="max-w-xs text-sm text-muted-foreground">
          Drop a CSV or PDF here, or use a sample to see how import works.
        </div>
      </div>
      <Button onclick={useSample}>
        <Upload class="size-4" /> Use sample statement
      </Button>
    </div>
  {:else if stage === "preview"}
    <!-- Column mapping -->
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="mb-3 text-sm font-medium">Map columns</div>
      <div class="grid grid-cols-3 gap-3">
        <label class="block space-y-1">
          <span class="text-xs font-medium text-muted-foreground">Date →</span>
          <select class={`${selectClass} w-full`} bind:value={mapDate}>
            {#each COLS as c (c)}<option value={c}>{c}</option>{/each}
          </select>
        </label>
        <label class="block space-y-1">
          <span class="text-xs font-medium text-muted-foreground">Merchant →</span>
          <select class={`${selectClass} w-full`} bind:value={mapMerchant}>
            {#each COLS as c (c)}<option value={c}>{c}</option>{/each}
          </select>
        </label>
        <label class="block space-y-1">
          <span class="text-xs font-medium text-muted-foreground">Amount →</span>
          <select class={`${selectClass} w-full`} bind:value={mapAmount}>
            {#each COLS as c (c)}<option value={c}>{c}</option>{/each}
          </select>
        </label>
      </div>
    </div>

    <!-- Preview table -->
    <div class="overflow-x-auto rounded-lg border border-border bg-card">
      <table class="w-full border-collapse text-sm">
        <thead>
          <tr class="border-b border-border text-left">
            <th class="px-3 py-1.5 font-medium text-muted-foreground">Date</th>
            <th class="px-3 py-1.5 font-medium text-muted-foreground">Description</th>
            <th class="px-3 py-1.5 text-right font-medium text-muted-foreground">Amount</th>
          </tr>
        </thead>
        <tbody>
          {#each SAMPLE as r (r.description)}
            <tr class="border-b border-border/50">
              <td class="px-3 py-1.5 font-mono tabular-nums text-foreground">{r.date}</td>
              <td class="px-3 py-1.5 break-words text-foreground">{r.description}</td>
              <td
                class={[
                  "px-3 py-1.5 text-right font-mono tabular-nums",
                  r.amount >= 0 ? "text-success" : "text-destructive",
                ].join(" ")}
              >
                {r.amount >= 0 ? "+" : "−"}{Math.abs(r.amount).toFixed(2)}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <div class="text-sm text-muted-foreground">
      {SAMPLE.length} rows will be auto-categorized using your rules.
    </div>

    <!-- Honest import-trust surface -->
    <div class="flex items-center gap-2 rounded-md bg-muted/50 px-3 py-2 text-xs text-muted-foreground">
      <Info class="size-3.5 shrink-0" />
      0 rows skipped, 0 malformed. Duplicates are detected on import.
    </div>

    <div class="flex items-center gap-2">
      <Button onclick={doImport} disabled={importing}>
        {importing ? "Importing…" : "Import"}
      </Button>
      <Button variant="ghost" size="sm" onclick={reset}>Cancel</Button>
    </div>
  {:else}
    <!-- Done state -->
    <div
      class="flex flex-col items-center justify-center gap-3 rounded-xl border border-border bg-card p-10 text-center"
    >
      <div class="rounded-full bg-success/15 p-3">
        <CheckCircle2 class="size-6 text-success" />
      </div>
      <div class="font-display text-base font-semibold">Imported &amp; categorized ✓</div>
      <div class="max-w-xs text-sm text-muted-foreground">
        {SAMPLE.length} transactions imported. Your dashboard is up to date.
      </div>
      <Button variant="outline" size="sm" onclick={reset}>Import another</Button>
    </div>
  {/if}
</div>
