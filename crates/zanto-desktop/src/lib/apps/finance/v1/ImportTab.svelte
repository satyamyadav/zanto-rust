<script lang="ts">
  import { ipc } from "$lib/ipc";
  import { Button } from "$lib/components/ui/button";
  import Upload from "@lucide/svelte/icons/upload";
  import CheckCircle2 from "@lucide/svelte/icons/check-circle-2";
  import Info from "@lucide/svelte/icons/info";
  import FileText from "@lucide/svelte/icons/file-text";

  // Real import flow: pick a file → finance_parse_statement → map columns →
  // preview → finance_import_statement (re-reads + imports server-side, dedups,
  // auto-categorizes via rules). Stays walkable in dev:mock via mock handlers.

  type Parsed = {
    headers: string[];
    preview: string[][];
    total_rows: number;
    truncated: boolean;
    malformed: number;
    suggested_mapping: { date?: string; merchant?: string; amount?: string; debit?: string; credit?: string; category?: string };
  };
  type ImportResult = { inserted: number; skipped: number; errors: any[]; total_rows?: number; truncated?: boolean; malformed?: number };
  type Account = { name: string; type?: string };

  let stage = $state<"choose" | "preview" | "done">("choose");
  let path = $state<string | null>(null);
  let parsed = $state<Parsed | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  // Column mapping (target field → header name, or "" = unmapped).
  let mapDate = $state("");
  let mapMerchant = $state("");
  let mapAmount = $state("");
  let mapDebit = $state("");
  let mapCredit = $state("");

  let accounts = $state<Account[]>([]);
  let account = $state("");
  let result = $state<ImportResult | null>(null);

  const NONE = "—";
  const headerOpts = $derived([NONE, ...(parsed?.headers ?? [])]);

  async function pickFile() {
    error = null;
    try {
      const paths = await ipc.pickFiles();
      if (!paths || paths.length === 0) return;
      await loadPath(paths[0]);
    } catch (e) {
      error = `${e}`;
    }
  }

  async function loadPath(p: string) {
    busy = true;
    error = null;
    try {
      const res: Parsed = await ipc.financeParseStatement(p);
      path = p;
      parsed = res;
      const sm = res.suggested_mapping ?? {};
      mapDate = sm.date ?? "";
      mapMerchant = sm.merchant ?? "";
      mapAmount = sm.amount ?? "";
      mapDebit = sm.debit ?? "";
      mapCredit = sm.credit ?? "";
      // Load accounts for the destination picker.
      const ar = (await ipc.queryApp("finance", "accounts")) as { accounts: Account[] };
      accounts = ar?.accounts ?? [];
      account = accounts[0]?.name ?? "";
      stage = "preview";
    } catch (e) {
      error = `${e}`;
    } finally {
      busy = false;
    }
  }

  const canImport = $derived(
    !!path && !!account && !!mapDate && (!!mapAmount || !!mapDebit || !!mapCredit),
  );

  async function doImport() {
    if (!path || !canImport) return;
    busy = true;
    error = null;
    try {
      const mapping: Record<string, string> = {};
      if (mapDate) mapping.date = mapDate;
      if (mapMerchant) mapping.merchant = mapMerchant;
      if (mapAmount) mapping.amount = mapAmount;
      if (mapDebit) mapping.debit = mapDebit;
      if (mapCredit) mapping.credit = mapCredit;
      result = await ipc.financeImportStatement(path, mapping, account);
      stage = "done";
    } catch (e) {
      error = `${e}`;
    } finally {
      busy = false;
    }
  }

  function reset() {
    stage = "choose";
    path = null;
    parsed = null;
    result = null;
    error = null;
  }

  const selectClass =
    "h-8 rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring";
</script>

<div class="space-y-4">
  {#if error}
    <div class="rounded-md border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive">{error}</div>
  {/if}

  {#if stage === "choose"}
    <div class="flex flex-col items-center justify-center gap-3 rounded-xl border-2 border-dashed border-border p-10 text-center">
      <div class="rounded-full bg-accent p-3"><Upload class="size-6 text-accent-foreground" /></div>
      <div class="space-y-1">
        <div class="font-display text-base font-semibold">Import a statement</div>
        <div class="max-w-xs text-sm text-muted-foreground">
          Choose a CSV or spreadsheet (xlsx/ods) export from your bank. It's read on
          your machine — nothing is uploaded.
        </div>
      </div>
      <Button onclick={pickFile} disabled={busy}>
        <Upload class="size-4" /> {busy ? "Reading…" : "Choose a file"}
      </Button>
    </div>
  {:else if stage === "preview" && parsed}
    <!-- Source file -->
    <div class="flex items-center gap-2 text-sm text-muted-foreground">
      <FileText class="size-4 shrink-0" />
      <span class="min-w-0 flex-1 truncate font-mono">{path}</span>
      <Button variant="ghost" size="sm" onclick={reset}>Change</Button>
    </div>

    <!-- Column mapping (real headers) + destination account -->
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="mb-3 text-sm font-medium">Map columns</div>
      <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
        {#each [["Date →", () => mapDate, (v: string) => (mapDate = v)], ["Merchant →", () => mapMerchant, (v: string) => (mapMerchant = v)], ["Amount →", () => mapAmount, (v: string) => (mapAmount = v)]] as [label, get, set] (label)}
          <label class="block space-y-1">
            <span class="text-xs font-medium text-muted-foreground">{label}</span>
            <select class={`${selectClass} w-full`} value={(get as () => string)() || NONE} onchange={(e) => (set as (v: string) => void)(e.currentTarget.value === NONE ? "" : e.currentTarget.value)}>
              {#each headerOpts as h (h)}<option value={h}>{h}</option>{/each}
            </select>
          </label>
        {/each}
        <label class="block space-y-1">
          <span class="text-xs font-medium text-muted-foreground">Account</span>
          {#if accounts.length}
            <select class={`${selectClass} w-full`} bind:value={account}>
              {#each accounts as a (a.name)}<option value={a.name}>{a.name}</option>{/each}
            </select>
          {:else}
            <div class="text-xs text-destructive">Create an account first (Accounts tab).</div>
          {/if}
        </label>
      </div>
      <div class="mt-2 text-[11px] text-muted-foreground">
        No single amount column? Map the bank's separate <em>debit</em>/<em>credit</em> columns instead — pick Amount = — and use those.
      </div>
    </div>

    <!-- Preview table (first rows, real data) -->
    <div class="overflow-x-auto rounded-lg border border-border bg-card">
      <table class="w-full border-collapse text-sm">
        <thead>
          <tr class="border-b border-border text-left">
            {#each parsed.headers as h (h)}
              <th class="px-3 py-1.5 font-medium text-muted-foreground">{h}</th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each parsed.preview.slice(0, 8) as row, i (i)}
            <tr class="border-b border-border/50">
              {#each parsed.headers as _h, c (c)}
                <td class="px-3 py-1.5 font-mono tabular-nums text-foreground">{row[c] ?? ""}</td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>

    <div class="text-sm text-muted-foreground">
      {parsed.total_rows} row{parsed.total_rows === 1 ? "" : "s"} will be imported and auto-categorized using your rules.
    </div>

    <!-- Honest import-trust surface -->
    {#if parsed.truncated || parsed.malformed > 0}
      <div class="flex items-center gap-2 rounded-md bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:bg-amber-950/30 dark:text-amber-300">
        <Info class="size-3.5 shrink-0" />
        {#if parsed.truncated}Capped at the first {parsed.preview.length}+ rows.{/if}
        {#if parsed.malformed > 0}{parsed.malformed} malformed row{parsed.malformed === 1 ? "" : "s"} were skipped.{/if}
      </div>
    {:else}
      <div class="flex items-center gap-2 rounded-md bg-muted/50 px-3 py-2 text-xs text-muted-foreground">
        <Info class="size-3.5 shrink-0" />
        Duplicates are detected and skipped on import.
      </div>
    {/if}

    <div class="flex items-center gap-2">
      <Button onclick={doImport} disabled={busy || !canImport}>
        {busy ? "Importing…" : "Import"}
      </Button>
      <Button variant="ghost" size="sm" onclick={reset}>Cancel</Button>
    </div>
  {:else if stage === "done" && result}
    <div class="flex flex-col items-center justify-center gap-3 rounded-xl border border-border bg-card p-10 text-center">
      <div class="rounded-full bg-success/15 p-3"><CheckCircle2 class="size-6 text-success" /></div>
      <div class="font-display text-base font-semibold">Imported &amp; categorized ✓</div>
      <div class="max-w-sm text-sm text-muted-foreground">
        {result.inserted} transaction{result.inserted === 1 ? "" : "s"} imported{result.skipped ? `, ${result.skipped} duplicate${result.skipped === 1 ? "" : "s"} skipped` : ""}.
        {#if (result.errors?.length ?? 0) > 0}{result.errors.length} row{result.errors.length === 1 ? "" : "s"} couldn't be read.{/if}
        Your dashboard is up to date.
      </div>
      <Button variant="outline" size="sm" onclick={reset}>Import another</Button>
    </div>
  {/if}
</div>
