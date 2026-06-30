<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { formatCurrency } from "../format";
  import AiEditButton from "../AiEditButton.svelte";
  import EditSheet from "../EditSheet.svelte";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Plus from "@lucide/svelte/icons/plus";
  import Search from "@lucide/svelte/icons/search";

  type Row = {
    id: number;
    date: string;
    merchant: string;
    category: string;
    amount: number;
    type: "income" | "expense";
    account: string;
    source: string;
  };

  let { initialFilter = "all" }: { initialFilter?: "all" | "uncategorized" } = $props();

  let rows = $state<Row[]>([]);
  let categories = $state<string[]>([]);
  let accounts = $state<string[]>([]);
  let currency = $state<string | undefined>(undefined);
  let error = $state<string | null>(null);
  let loading = $state(false);

  // svelte-ignore state_referenced_locally
  let onlyUncategorized = $state(initialFilter === "uncategorized");
  let searchText = $state("");
  let categoryFilter = $state("all");
  let editing = $state(false);
  let selected = $state<Set<number>>(new Set());
  let bulkCategory = $state("");

  async function load() {
    loading = true;
    error = null;
    try {
      const [tx, cats, ov] = await Promise.all([
        ipc.queryApp("finance", "list_transactions"),
        ipc.queryApp("finance", "categories"),
        ipc.queryApp("finance", "overview"),
      ]);
      rows = (tx?.rows ?? []) as Row[];
      categories = (cats ?? []) as string[];
      currency = ov?.currency;
      accounts = (ov?.accounts ?? []).map((a: { name: string }) => a.name);
      bulkCategory = categories[0] ?? "";
    } catch (e) {
      error = `${e}`;
    } finally {
      loading = false;
    }
  }
  onMount(load);

  const filtered = $derived(
    rows.filter((r) => {
      if (onlyUncategorized && r.category !== "uncategorized") return false;
      if (categoryFilter !== "all" && r.category !== categoryFilter) return false;
      if (searchText && !r.merchant.toLowerCase().includes(searchText.toLowerCase())) return false;
      return true;
    }),
  );

  function categoryOptions(current: string): string[] {
    const opts = [...categories];
    if (current && !opts.includes(current)) opts.unshift(current);
    if (!opts.includes("uncategorized")) opts.push("uncategorized");
    return opts;
  }

  async function setRowCategory(id: number, category: string) {
    try {
      await ipc.runAppAction("finance", "update_transaction", { id, category });
      await load();
    } catch (e) {
      error = `${e}`;
    }
  }

  function toggleSelected(id: number) {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selected = next;
  }

  async function applyBulk() {
    if (!selected.size || !bulkCategory) return;
    try {
      await ipc.runAppAction("finance", "categorize_transactions", {
        ids: [...selected],
        category: bulkCategory,
      });
      selected = new Set();
      await load();
    } catch (e) {
      error = `${e}`;
    }
  }

  // ── Add transaction overlay ─────────────────────────────────────────────────
  let addOpen = $state(false);
  let saving = $state(false);
  let fMerchant = $state("");
  let fAmount = $state("");
  let fType = $state<"income" | "expense">("expense");
  let fCategory = $state("");
  let fDate = $state("2026-06-28");
  let fAccount = $state("");

  function openAdd() {
    fMerchant = "";
    fAmount = "";
    fType = "expense";
    fCategory = categories[0] ?? "";
    fDate = "2026-06-28";
    fAccount = accounts[0] ?? "Checking";
    addOpen = true;
  }

  const canAdd = $derived(!!fMerchant.trim() && Number(fAmount) > 0);

  async function addTransaction() {
    if (!canAdd) return;
    saving = true;
    try {
      await ipc.runAppAction("finance", "add_transaction", {
        merchant: fMerchant.trim(),
        amount: Number(fAmount),
        type: fType,
        category: fCategory,
        date: fDate,
        account: fAccount,
      });
      addOpen = false;
      await load();
    } finally {
      saving = false;
    }
  }

  const selectClass =
    "h-7 rounded-md border border-border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring";

  function chipClass(active: boolean): string {
    return [
      "inline-flex items-center rounded-md px-2.5 py-1 text-sm font-medium outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring",
      active ? "bg-background text-foreground shadow-sm ring-1 ring-primary" : "text-muted-foreground hover:text-foreground",
    ].join(" ");
  }
</script>

<div class="space-y-3">
  <!-- Controls -->
  <div class="flex flex-wrap items-center gap-2">
    <div class="relative">
      <Search class="pointer-events-none absolute left-2 top-1/2 size-3.5 -translate-y-1/2 text-muted-foreground" />
      <Input
        class="h-8 w-48 pl-7"
        bind:value={searchText}
        placeholder="Search merchant…"
        aria-label="Search merchant"
      />
    </div>

    <select
      class="h-8 rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
      bind:value={categoryFilter}
      aria-label="Filter by category"
    >
      <option value="all">All categories</option>
      {#each categories as c (c)}
        <option value={c}>{c}</option>
      {/each}
      <option value="uncategorized">uncategorized</option>
    </select>

    <button
      type="button"
      class={chipClass(onlyUncategorized)}
      onclick={() => (onlyUncategorized = !onlyUncategorized)}
    >
      Uncategorized
    </button>

    <div class="ml-auto flex items-center gap-2">
      <Button
        variant={editing ? "default" : "outline"}
        size="sm"
        onclick={() => (editing = !editing)}
      >
        <Pencil class="size-3.5" /> Edit
      </Button>
      <Button size="sm" onclick={openAdd}>
        <Plus class="size-3.5" /> Add transaction
      </Button>
    </div>
  </div>

  <!-- Bulk bar -->
  {#if editing && selected.size}
    <div class="flex items-center gap-2 rounded-md border border-border bg-muted/40 px-3 py-2 text-sm">
      <span class="text-muted-foreground">{selected.size} selected</span>
      <select class={selectClass} bind:value={bulkCategory} aria-label="Bulk category">
        {#each categories as c (c)}
          <option value={c}>{c}</option>
        {/each}
      </select>
      <Button size="sm" variant="outline" onclick={applyBulk}>Set category for selected</Button>
    </div>
  {/if}

  {#if error}
    <div class="text-sm text-destructive">Couldn't update transactions: {error}.</div>
  {/if}

  {#if loading}
    <div class="h-40 animate-pulse rounded-lg border border-border bg-muted/40"></div>
  {:else if filtered.length === 0}
    <div class="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
      No transactions match.
    </div>
  {:else}
    <div class="overflow-x-auto rounded-lg border border-border bg-card">
      <table class="w-full border-collapse text-sm">
        <thead>
          <tr class="border-b border-border text-left">
            {#if editing}
              <th class="w-8 px-3 py-1.5"></th>
            {/if}
            <th class="px-3 py-1.5 font-medium text-muted-foreground">Date</th>
            <th class="px-3 py-1.5 font-medium text-muted-foreground">Merchant</th>
            <th class="px-3 py-1.5 font-medium text-muted-foreground">Category</th>
            <th class="px-3 py-1.5 font-medium text-muted-foreground">Account</th>
            <th class="px-3 py-1.5 text-right font-medium text-muted-foreground">Amount</th>
            {#if editing}
              <th class="w-8 px-3 py-1.5"></th>
            {/if}
          </tr>
        </thead>
        <tbody>
          {#each filtered as r (r.id)}
            <tr class="border-b border-border/50">
              {#if editing}
                <td class="px-3 py-1.5">
                  <input
                    type="checkbox"
                    checked={selected.has(r.id)}
                    onchange={() => toggleSelected(r.id)}
                    aria-label={`Select ${r.merchant}`}
                  />
                </td>
              {/if}
              <td class="px-3 py-1.5 font-mono tabular-nums text-foreground">{r.date}</td>
              <td class="px-3 py-1.5 break-words text-foreground">{r.merchant}</td>
              <td class="px-3 py-1.5">
                {#if editing}
                  <select
                    class={selectClass}
                    value={r.category}
                    onchange={(e) => setRowCategory(r.id, (e.currentTarget as HTMLSelectElement).value)}
                    aria-label="Category"
                  >
                    {#each categoryOptions(r.category) as c (c)}
                      <option value={c}>{c}</option>
                    {/each}
                  </select>
                {:else}
                  <span class="text-muted-foreground">{r.category}</span>
                {/if}
              </td>
              <td class="px-3 py-1.5 text-muted-foreground">{r.account}</td>
              <td
                class={[
                  "px-3 py-1.5 text-right font-mono tabular-nums",
                  r.type === "income" ? "text-success" : "text-destructive",
                ].join(" ")}
              >
                {r.type === "income" ? "+" : "−"}{formatCurrency(r.amount, currency)}
              </td>
              {#if editing}
                <td class="px-3 py-1.5 text-right">
                  <AiEditButton
                    prompt={`Recategorize the '${r.merchant}' transaction and similar ones`}
                  />
                </td>
              {/if}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<!-- Add transaction overlay -->
<EditSheet
  bind:open={addOpen}
  title="Add transaction"
  onSave={addTransaction}
  canSave={canAdd}
  {saving}
>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <div class="text-xs text-muted-foreground">Record a transaction manually.</div>
      <AiEditButton prompt="Add a transaction" size="md" />
    </div>
    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Merchant</span>
      <Input bind:value={fMerchant} placeholder="Whole Foods" />
    </label>
    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Amount</span>
      <Input type="number" min="0" step="0.01" bind:value={fAmount} placeholder="42.00" />
    </label>
    <div class="grid grid-cols-2 gap-3">
      <label class="block space-y-1">
        <span class="text-xs font-medium text-muted-foreground">Type</span>
        <select
          class="h-9 w-full rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
          bind:value={fType}
        >
          <option value="expense">expense</option>
          <option value="income">income</option>
        </select>
      </label>
      <label class="block space-y-1">
        <span class="text-xs font-medium text-muted-foreground">Category</span>
        <select
          class="h-9 w-full rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
          bind:value={fCategory}
        >
          {#each categories as c (c)}
            <option value={c}>{c}</option>
          {/each}
        </select>
      </label>
    </div>
    <div class="grid grid-cols-2 gap-3">
      <label class="block space-y-1">
        <span class="text-xs font-medium text-muted-foreground">Date</span>
        <Input type="date" bind:value={fDate} />
      </label>
      <label class="block space-y-1">
        <span class="text-xs font-medium text-muted-foreground">Account</span>
        <select
          class="h-9 w-full rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
          bind:value={fAccount}
        >
          {#each (accounts.length ? accounts : ["Checking"]) as a (a)}
            <option value={a}>{a}</option>
          {/each}
        </select>
      </label>
    </div>
  </div>
</EditSheet>
