<script lang="ts">
  import { ipc, type FileEntry } from "$lib/ipc";
  import { Button } from "$lib/components/ui/button";
  import { toast } from "svelte-sonner";
  import {
    Folder,
    FileText,
    ArrowUp,
    RefreshCw,
    FileUp,
    ChevronDown,
    ChevronRight,
    Check,
  } from "@lucide/svelte";

  // T4 — statement import. State machine: pick file → parse → review/map →
  // import → result. The path is permission-checked server-side; the file
  // picker reuses the B1 `browse_dir` pattern from ResourcesPanel.

  let { onImported }: { onImported?: () => void } = $props();

  type ParseResult = {
    columns: string[];
    headers: string[];
    preview: string[][];
    rows: string[][];
    row_count: number;
    suggested_mapping: {
      date?: string;
      merchant?: string;
      category?: string;
      debit?: string;
      credit?: string;
      amount?: string;
    };
  };
  type Account = { name: string; type: string; opening_balance: number };
  type ImportResult = {
    inserted: number;
    skipped: number;
    errors: { row: number; reason: string }[];
  };

  type Stage = "pick" | "review" | "result";
  let stage = $state<Stage>("pick");

  // pick
  let path = $state("");
  let parsing = $state(false);
  let parseError = $state<string | null>(null);

  // file picker (browse_dir)
  let browsing = $state(false);
  let entries = $state<FileEntry[]>([]);
  let cwd = $state<string | null>(null);
  let trail = $state<string[]>([]);
  let browseError = $state<string | null>(null);
  let browseLoading = $state(false);

  // review
  let parsed = $state<ParseResult | null>(null);
  let accounts = $state<Account[]>([]);
  let account = $state("");
  let importing = $state(false);

  // Target mapping fields → chosen header (or "—" for none).
  const FIELDS = [
    { key: "date", label: "Date" },
    { key: "merchant", label: "Merchant" },
    { key: "category", label: "Category" },
    { key: "debit", label: "Debit" },
    { key: "credit", label: "Credit" },
    { key: "amount", label: "Amount" },
  ] as const;
  type FieldKey = (typeof FIELDS)[number]["key"];
  let mapSel = $state<Record<FieldKey, string>>({
    date: "—",
    merchant: "—",
    category: "—",
    debit: "—",
    credit: "—",
    amount: "—",
  });

  // result
  let result = $state<ImportResult | null>(null);
  let showErrors = $state(false);

  async function parse() {
    if (!path.trim()) return;
    parsing = true;
    parseError = null;
    try {
      const res = (await ipc.financeParseStatement(path.trim())) as ParseResult;
      parsed = res;
      const sm = res.suggested_mapping ?? {};
      mapSel = {
        date: sm.date ?? "—",
        merchant: sm.merchant ?? "—",
        category: sm.category ?? "—",
        debit: sm.debit ?? "—",
        credit: sm.credit ?? "—",
        amount: sm.amount ?? "—",
      };
      // Load accounts; default to the first.
      try {
        const ar = (await ipc.queryApp("finance", "accounts")) as { accounts: Account[] };
        accounts = ar?.accounts ?? [];
        account = accounts[0]?.name ?? "";
      } catch {
        accounts = [];
        account = "";
      }
      stage = "review";
    } catch (e) {
      parseError = `${e}`;
    } finally {
      parsing = false;
    }
  }

  async function browse(p: string | null) {
    browseLoading = true;
    browseError = null;
    try {
      entries = await ipc.browseDir(p ?? undefined);
      cwd = p;
    } catch (e) {
      browseError = `${e}`;
    } finally {
      browseLoading = false;
    }
  }

  function toggleBrowse() {
    browsing = !browsing;
    if (browsing && entries.length === 0) browse(null);
  }

  function openDir(e: FileEntry) {
    if (cwd !== null) trail = [...trail, cwd];
    browse(e.path);
  }

  function up() {
    if (trail.length === 0) {
      browse(null);
      return;
    }
    const prev = trail[trail.length - 1];
    trail = trail.slice(0, -1);
    browse(prev);
  }

  function pickFile(e: FileEntry) {
    path = e.path;
    browsing = false;
  }

  const atRoot = $derived(cwd === null);
  const dirs = $derived(entries.filter((e) => e.isDir));
  const files = $derived(entries.filter((e) => !e.isDir));

  async function runImport() {
    if (!parsed) return;
    const mapping: Record<string, string> = {};
    for (const f of FIELDS) {
      const v = mapSel[f.key];
      if (v && v !== "—") mapping[f.key] = v;
    }
    importing = true;
    try {
      const res = (await ipc.runAppAction("finance", "import_transactions", {
        headers: parsed.headers,
        rows: parsed.rows,
        mapping,
        account,
      })) as ImportResult;
      result = res;
      showErrors = false;
      stage = "result";
      toast.success(`Imported ${res.inserted}, skipped ${res.skipped} duplicates`);
      onImported?.();
    } catch (e) {
      toast.error("Import failed", { description: `${e}` });
    } finally {
      importing = false;
    }
  }

  function reset() {
    stage = "pick";
    path = "";
    parsed = null;
    parseError = null;
    result = null;
    showErrors = false;
    browsing = false;
  }

  const selectClass =
    "h-8 w-full rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring";
</script>

<div class="space-y-4">
  {#if stage === "pick"}
    <div class="space-y-3">
      <div class="font-display text-sm font-semibold">Import a statement</div>
      <div class="flex items-center gap-2">
        <input
          type="text"
          bind:value={path}
          placeholder="/absolute/path/to/statement.csv"
          class="h-9 min-w-0 flex-1 rounded-md border border-border bg-background px-3 font-mono text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
        <Button variant="outline" size="sm" onclick={toggleBrowse}>
          <Folder /> Browse
        </Button>
        <Button size="sm" onclick={parse} disabled={!path.trim() || parsing}>
          <FileUp /> {parsing ? "Parsing…" : "Parse"}
        </Button>
      </div>

      {#if parseError}
        <div class="text-sm text-destructive">Couldn't parse this file: {parseError}.</div>
      {/if}

      {#if browsing}
        <div class="space-y-2 rounded-lg border border-border bg-card p-3">
          <div class="flex items-center gap-2 text-xs text-muted-foreground">
            <Button
              variant="outline"
              size="xs"
              onclick={up}
              disabled={atRoot}
              aria-label="Go up one level"
            >
              <ArrowUp /> Up
            </Button>
            <span class="min-w-0 flex-1 truncate font-mono">{cwd ?? "Allowed roots"}</span>
            <Button variant="outline" size="xs" onclick={() => browse(cwd)} disabled={browseLoading}>
              <RefreshCw /> Refresh
            </Button>
          </div>

          {#if browseError}
            <div class="text-sm text-destructive">Couldn't browse this folder: {browseError}.</div>
          {:else if browseLoading}
            <div class="h-32 animate-pulse rounded-md bg-muted/40"></div>
          {:else if entries.length === 0}
            <div class="rounded-md border border-dashed border-border p-3 text-sm text-muted-foreground">
              Nothing here.
            </div>
          {:else}
            <ul class="max-h-64 divide-y divide-border/50 overflow-y-auto rounded-md border border-border">
              {#each dirs as d (d.path)}
                <li>
                  <button
                    type="button"
                    class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
                    onclick={() => openDir(d)}
                  >
                    <Folder class="size-4 text-muted-foreground" />
                    <span class="min-w-0 flex-1 truncate font-mono">{d.name}</span>
                    <span class="text-xs text-muted-foreground">Folder</span>
                  </button>
                </li>
              {/each}
              {#each files as f (f.path)}
                <li>
                  <button
                    type="button"
                    class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
                    onclick={() => pickFile(f)}
                  >
                    <FileText class="size-4 text-muted-foreground" />
                    <span class="min-w-0 flex-1 truncate font-mono">{f.name}</span>
                    <span class="text-xs text-muted-foreground">Select</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        </div>
      {/if}
    </div>
  {:else if stage === "review" && parsed}
    <div class="space-y-4">
      <div class="flex items-center justify-between">
        <div class="font-display text-sm font-semibold">Review &amp; map columns</div>
        <Button variant="ghost" size="sm" onclick={reset}>Change file</Button>
      </div>

      <div class="space-y-2 rounded-lg border border-border bg-card p-3">
        <div class="text-xs font-medium text-muted-foreground">Column mapping</div>
        <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
          {#each FIELDS as f (f.key)}
            <label class="space-y-1 text-sm">
              <span class="text-muted-foreground">{f.label}</span>
              <select class={selectClass} bind:value={mapSel[f.key]} aria-label={f.label}>
                <option value="—">—</option>
                {#each parsed.columns as c (c)}
                  <option value={c}>{c}</option>
                {/each}
              </select>
            </label>
          {/each}
        </div>
        <p class="text-xs text-muted-foreground">
          Map Debit+Credit OR a single signed Amount column.
        </p>
      </div>

      <label class="block space-y-1 text-sm">
        <span class="text-muted-foreground">Account</span>
        {#if accounts.length}
          <select class={selectClass} bind:value={account} aria-label="Account">
            {#each accounts as a (a.name)}
              <option value={a.name}>{a.name}</option>
            {/each}
          </select>
        {:else}
          <div class="text-sm text-muted-foreground">No accounts configured.</div>
        {/if}
      </label>

      <div class="space-y-2">
        <div class="text-xs font-medium text-muted-foreground">
          Preview (first {Math.min(10, parsed.preview.length)} of {parsed.row_count} rows)
        </div>
        <div class="overflow-x-auto rounded-lg border border-border bg-card">
          <table class="w-full border-collapse text-sm">
            <thead>
              <tr class="border-b border-border text-left">
                {#each parsed.headers as h (h)}
                  <th scope="col" class="whitespace-nowrap px-3 py-1.5 font-medium text-muted-foreground">
                    {h}
                  </th>
                {/each}
              </tr>
            </thead>
            <tbody>
              {#each parsed.preview.slice(0, 10) as row, i (i)}
                <tr class="border-b border-border/50">
                  {#each row as cell, j (j)}
                    <td class="whitespace-nowrap px-3 py-1.5 font-mono tabular-nums text-foreground">
                      {cell}
                    </td>
                  {/each}
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      </div>

      <Button onclick={runImport} disabled={importing}>
        <FileUp /> {importing ? "Importing…" : `Import ${parsed.row_count} rows`}
      </Button>
    </div>
  {:else if stage === "result" && result}
    <div class="space-y-3">
      <div class="flex items-center gap-2 rounded-md bg-success/10 px-3 py-2 text-sm text-success">
        <Check class="size-4 shrink-0" />
        <span>Imported {result.inserted}, skipped {result.skipped} duplicates</span>
      </div>

      {#if result.errors.length}
        <div class="rounded-lg border border-border bg-card">
          <button
            type="button"
            class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm font-medium outline-none hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring"
            onclick={() => (showErrors = !showErrors)}
          >
            {#if showErrors}
              <ChevronDown class="size-4" />
            {:else}
              <ChevronRight class="size-4" />
            {/if}
            <span class="text-destructive">{result.errors.length} row{result.errors.length === 1 ? "" : "s"} skipped with errors</span>
          </button>
          {#if showErrors}
            <ul class="divide-y divide-border/50 border-t border-border">
              {#each result.errors.slice(0, 20) as err (err.row)}
                <li class="px-3 py-1.5 text-sm">
                  <span class="font-mono text-muted-foreground">row {err.row}:</span>
                  {err.reason}
                </li>
              {/each}
              {#if result.errors.length > 20}
                <li class="px-3 py-1.5 text-xs text-muted-foreground">
                  +{result.errors.length - 20} more
                </li>
              {/if}
            </ul>
          {/if}
        </div>
      {/if}

      <Button variant="outline" onclick={reset}>Import another</Button>
    </div>
  {/if}
</div>
