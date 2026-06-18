<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { confirm } from "$lib/stores/confirm.svelte";
  import { formatCurrency } from "./format";
  import { Pencil, Trash2, Check, X } from "@lucide/svelte";

  type Row = {
    id: number;
    type: string;
    date: string;
    amount: number;
    merchant: string;
    category: string;
    note?: string;
    source?: string;
    account?: string;
  };

  let { currency, categories = [], accounts = [], initialFilter = "all" }: {
    currency?: string;
    categories?: string[];
    accounts?: string[];
    initialFilter?: "all" | "uncategorized";
  } = $props();

  // The parent remounts this component (via `{#key}`) to change the initial
  // filter, so capturing the initial value here is intentional.
  // svelte-ignore state_referenced_locally
  let filter = $state<"all" | "uncategorized">(initialFilter);
  let rows = $state<Row[]>([]);
  let error = $state<string | null>(null);
  let loading = $state(false);

  // Inline edit state — only one row at a time.
  let editId = $state<number | null>(null);
  let editCategory = $state("");
  let editAmount = $state(0);
  let editType = $state<"income" | "expense">("expense");
  let editAccount = $state("");
  let saving = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      const args = filter === "uncategorized" ? { category: "uncategorized" } : {};
      const res = await ipc.queryApp("finance", "list_transactions", args);
      rows = (res?.rows ?? []) as Row[];
    } catch (e) {
      error = `${e}`;
    } finally {
      loading = false;
    }
  }

  function setFilter(f: "all" | "uncategorized") {
    if (filter === f) return;
    filter = f;
    cancelEdit();
    load();
  }

  function startEdit(r: Row) {
    editId = r.id;
    editCategory = r.category;
    editAmount = r.amount;
    editType = r.type === "income" ? "income" : "expense";
    editAccount = r.account ?? "Cash";
  }

  function cancelEdit() {
    editId = null;
  }

  async function saveEdit(id: number) {
    saving = true;
    error = null;
    try {
      await ipc.runAppAction("finance", "update_transaction", {
        id,
        category: editCategory,
        amount: Number(editAmount),
        type: editType,
        account: editAccount,
      });
      editId = null;
      // Backend re-resolves the category against the profile + rules, so reload.
      await load();
    } catch (e) {
      error = `${e}`;
    } finally {
      saving = false;
    }
  }

  async function remove(id: number) {
    if (
      !(await confirm({
        title: "Delete transaction?",
        body: "This permanently removes the transaction.",
        confirmLabel: "Delete",
        destructive: true,
      }))
    )
      return;
    error = null;
    try {
      await ipc.runAppAction("finance", "delete_transaction", { id });
      await load();
    } catch (e) {
      error = `${e}`;
    }
  }

  // Options for the edit category select: profile categories, plus the row's
  // current value if missing, plus "uncategorized".
  function categoryOptions(current: string): string[] {
    const opts = [...categories];
    if (current && !opts.includes(current)) opts.unshift(current);
    if (!opts.includes("uncategorized")) opts.push("uncategorized");
    return opts;
  }

  // Account select options: the passed account names, plus the row's current
  // account if missing, with "Cash" as a default fallback.
  function accountOptions(current: string): string[] {
    const opts = accounts.length ? [...accounts] : ["Cash"];
    if (current && !opts.includes(current)) opts.unshift(current);
    return opts;
  }

  function chipClass(active: boolean): string {
    return [
      "inline-flex items-center rounded-md px-2.5 py-1 text-sm font-medium outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring",
      active
        ? "bg-background text-foreground shadow-sm ring-1 ring-primary"
        : "text-muted-foreground hover:text-foreground",
    ].join(" ");
  }

  const selectClass =
    "h-7 rounded-md border border-border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring";
  const iconBtnClass =
    "rounded-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30";

  onMount(load);
</script>

<div class="space-y-3">
  <div class="inline-flex items-center gap-1 rounded-lg border border-border bg-muted/40 p-0.5">
    <button type="button" class={chipClass(filter === "all")} onclick={() => setFilter("all")}>
      All
    </button>
    <button
      type="button"
      class={chipClass(filter === "uncategorized")}
      onclick={() => setFilter("uncategorized")}
    >
      Uncategorized
    </button>
  </div>

  {#if error}
    <div class="text-sm text-destructive">Couldn't update transactions: {error}. Try again.</div>
  {/if}

  {#if loading}
    <div class="h-40 animate-pulse rounded-lg border border-border bg-muted/40"></div>
  {:else if rows.length === 0}
    <div class="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
      {filter === "uncategorized"
        ? "Nothing uncategorized — you're all caught up."
        : "No transactions."}
    </div>
  {:else}
    <div class="overflow-x-auto rounded-lg border border-border bg-card">
      <table class="w-full border-collapse text-sm">
        <thead>
          <tr class="border-b border-border text-left">
            <th scope="col" class="px-3 py-1.5 font-medium text-muted-foreground">Date</th>
            <th scope="col" class="px-3 py-1.5 font-medium text-muted-foreground">Merchant</th>
            <th scope="col" class="px-3 py-1.5 font-medium text-muted-foreground">Category</th>
            <th scope="col" class="px-3 py-1.5 font-medium text-muted-foreground">Account</th>
            <th scope="col" class="px-3 py-1.5 font-medium text-muted-foreground">Type</th>
            <th scope="col" class="px-3 py-1.5 text-right font-medium text-muted-foreground">Amount</th>
            <th scope="col" class="px-3 py-1.5 text-right font-medium text-muted-foreground"></th>
          </tr>
        </thead>
        <tbody>
          {#each rows as r (r.id)}
            {#if editId === r.id}
              <tr class="border-b border-border/50 bg-muted/30">
                <td class="px-3 py-1.5 font-mono tabular-nums text-foreground">{r.date}</td>
                <td class="px-3 py-1.5 break-words text-foreground">{r.merchant}</td>
                <td class="px-3 py-1.5">
                  <select class={selectClass} bind:value={editCategory} aria-label="Category">
                    {#each categoryOptions(r.category) as c (c)}
                      <option value={c}>{c}</option>
                    {/each}
                  </select>
                </td>
                <td class="px-3 py-1.5">
                  <select class={selectClass} bind:value={editAccount} aria-label="Account">
                    {#each accountOptions(r.account ?? "Cash") as a (a)}
                      <option value={a}>{a}</option>
                    {/each}
                  </select>
                </td>
                <td class="px-3 py-1.5">
                  <select class={selectClass} bind:value={editType} aria-label="Type">
                    <option value="expense">expense</option>
                    <option value="income">income</option>
                  </select>
                </td>
                <td class="px-3 py-1.5 text-right">
                  <input
                    type="number"
                    class="h-7 w-24 rounded-md border border-border bg-background px-2 text-right text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    bind:value={editAmount}
                    aria-label="Amount"
                  />
                </td>
                <td class="px-3 py-1.5">
                  <div class="flex items-center justify-end gap-2">
                    <button
                      type="button"
                      class="rounded-sm text-muted-foreground outline-none hover:text-success focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-30"
                      onclick={() => saveEdit(r.id)}
                      disabled={saving}
                      aria-label="Save"
                    >
                      <Check class="size-4" />
                    </button>
                    <button
                      type="button"
                      class={iconBtnClass}
                      onclick={cancelEdit}
                      disabled={saving}
                      aria-label="Cancel"
                    >
                      <X class="size-4" />
                    </button>
                  </div>
                </td>
              </tr>
            {:else}
              <tr class="border-b border-border/50">
                <td class="px-3 py-1.5 font-mono tabular-nums text-foreground">{r.date}</td>
                <td class="px-3 py-1.5 break-words text-foreground">{r.merchant}</td>
                <td class="px-3 py-1.5 text-muted-foreground">{r.category}</td>
                <td class="px-3 py-1.5 text-muted-foreground">{r.account ?? "Cash"}</td>
                <td class="px-3 py-1.5 text-muted-foreground">{r.type}</td>
                <td
                  class={[
                    "px-3 py-1.5 text-right font-mono tabular-nums",
                    r.type === "income" ? "text-success" : "text-destructive",
                  ].join(" ")}
                >
                  {r.type === "income" ? "+" : "−"}{formatCurrency(r.amount, currency)}
                </td>
                <td class="px-3 py-1.5">
                  <div class="flex items-center justify-end gap-2">
                    <button
                      type="button"
                      class={iconBtnClass}
                      onclick={() => startEdit(r)}
                      aria-label="Edit transaction"
                    >
                      <Pencil class="size-4" />
                    </button>
                    <button
                      type="button"
                      class="rounded-sm text-muted-foreground outline-none hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring"
                      onclick={() => remove(r.id)}
                      aria-label="Delete transaction"
                    >
                      <Trash2 class="size-4" />
                    </button>
                  </div>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>
