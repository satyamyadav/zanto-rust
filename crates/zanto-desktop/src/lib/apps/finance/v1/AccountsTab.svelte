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

  type Account = { name: string; type: string; balance: number };

  let accounts = $state<Account[]>([]);
  let currency = $state<string | undefined>(undefined);
  let error = $state<string | null>(null);
  let loading = $state(false);
  let editing = $state(false);

  const netWorth = $derived(accounts.reduce((s, a) => s + a.balance, 0));

  async function load() {
    loading = true;
    error = null;
    try {
      const [res, ov] = await Promise.all([
        ipc.queryApp("finance", "accounts"),
        ipc.queryApp("finance", "overview"),
      ]);
      accounts = (res?.accounts ?? []) as Account[];
      currency = ov?.currency;
    } catch (e) {
      error = `${e}`;
    } finally {
      loading = false;
    }
  }
  onMount(load);

  // ── Account editor overlay ──────────────────────────────────────────────────
  let sheetOpen = $state(false);
  let sheetTitle = $state("Add account");
  let editIndex = $state<number | null>(null); // null = new
  let fName = $state("");
  let fType = $state("checking");
  let fOpening = $state("");
  let saving = $state(false);

  const ACCOUNT_TYPES = ["checking", "savings", "credit", "cash", "investment"];

  function openNew() {
    editIndex = null;
    sheetTitle = "Add account";
    fName = "";
    fType = "checking";
    fOpening = "";
    sheetOpen = true;
  }

  function openEdit(i: number) {
    const a = accounts[i];
    editIndex = i;
    sheetTitle = "Edit account";
    fName = a.name;
    fType = a.type;
    fOpening = String(a.balance);
    sheetOpen = true;
  }

  const canSave = $derived(!!fName.trim());

  async function save() {
    if (!canSave) return;
    saving = true;
    try {
      // Rebuild the full account list (mock save_accounts replaces wholesale).
      const list = accounts.map((a) => ({
        name: a.name,
        type: a.type,
        opening_balance: a.balance,
      }));
      const entry = {
        name: fName.trim(),
        type: fType,
        opening_balance: Number(fOpening) || 0,
      };
      if (editIndex === null) list.push(entry);
      else list[editIndex] = entry;
      await ipc.runAppAction("finance", "save_accounts", { accounts: list });
      sheetOpen = false;
      await load();
    } finally {
      saving = false;
    }
  }
</script>

<div class="space-y-4">
  <!-- Net worth headline -->
  <div class="flex items-start justify-between gap-3 rounded-lg border border-border bg-card p-4">
    <div>
      <div class="text-xs text-muted-foreground">Net worth</div>
      <div class="mt-1 font-display text-3xl font-semibold tabular-nums">
        {formatCurrency(netWorth, currency)}
      </div>
    </div>
    <div class="flex items-center gap-2">
      <Button
        variant={editing ? "default" : "outline"}
        size="sm"
        onclick={() => (editing = !editing)}
      >
        <Pencil class="size-3.5" /> Edit
      </Button>
      <Button size="sm" onclick={openNew}>
        <Plus class="size-3.5" /> Account
      </Button>
    </div>
  </div>

  {#if error}
    <div class="text-sm text-destructive">Couldn't load accounts: {error}.</div>
  {/if}

  {#if loading}
    <div class="grid grid-cols-2 gap-3">
      {#each Array(2) as _, i (i)}
        <div class="h-24 animate-pulse rounded-lg border border-border bg-muted/40"></div>
      {/each}
    </div>
  {:else if accounts.length === 0}
    <div class="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
      No accounts yet. Add one to get started.
    </div>
  {:else}
    <div class="grid grid-cols-2 gap-3">
      {#each accounts as a, i (a.name)}
        <div class="rounded-lg border border-border bg-card p-3">
          <div class="flex items-start justify-between gap-2">
            <div class="min-w-0">
              <div class="break-words text-sm font-medium">{a.name}</div>
              <div class="text-xs capitalize text-muted-foreground">{a.type}</div>
            </div>
            {#if editing}
              <div class="flex shrink-0 items-center gap-1.5">
                <button
                  type="button"
                  class="rounded-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
                  onclick={() => openEdit(i)}
                  aria-label="Edit account"
                >
                  <Pencil class="size-3.5" />
                </button>
                <AiEditButton prompt="Add a new account" />
              </div>
            {/if}
          </div>
          <div
            class={[
              "mt-2 font-display text-xl font-semibold tabular-nums",
              a.balance < 0 ? "text-destructive" : "",
            ].join(" ")}
          >
            {formatCurrency(a.balance, currency)}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- Account editor overlay -->
<EditSheet bind:open={sheetOpen} title={sheetTitle} onSave={save} {canSave} {saving}>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <div class="text-xs text-muted-foreground">Name, type and opening balance.</div>
      <AiEditButton prompt="Add a new account" size="md" />
    </div>
    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Name</span>
      <Input bind:value={fName} placeholder="Checking" />
    </label>
    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Type</span>
      <select
        class="h-9 w-full rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
        bind:value={fType}
      >
        {#each ACCOUNT_TYPES as t (t)}
          <option value={t}>{t}</option>
        {/each}
      </select>
    </label>
    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Opening balance</span>
      <Input type="number" step="0.01" bind:value={fOpening} placeholder="0" />
    </label>
  </div>
</EditSheet>
