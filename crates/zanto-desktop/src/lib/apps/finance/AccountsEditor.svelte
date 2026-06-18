<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Check, Plus, Trash2 } from "@lucide/svelte";

  // `_id` is a stable per-row client key so bound inputs don't re-associate to the
  // wrong row after a remove (B4-2). It is never sent to the backend.
  type AccountRow = { _id: number; name: string; type: string; opening_balance: number | string };

  let rowSeq = 0;
  const nextId = () => ++rowSeq;

  let { onSaved }: { onSaved?: () => void } = $props();

  let rows = $state<AccountRow[]>([]);
  let error = $state<string | null>(null);
  let saving = $state(false);
  let saved = $state(false);

  const TYPES = ["checking", "savings", "card", "cash"];

  async function load() {
    error = null;
    try {
      const res: { accounts?: AccountRow[] } = await ipc.queryApp("finance", "accounts");
      rows = (res?.accounts ?? []).map((a) => ({
        _id: nextId(),
        name: a.name,
        type: a.type,
        opening_balance: a.opening_balance,
      }));
    } catch (e) {
      error = `${e}`;
    }
  }

  function add() {
    rows = [...rows, { _id: nextId(), name: "", type: "checking", opening_balance: "" }];
  }

  function remove(i: number) {
    rows = rows.filter((_, j) => j !== i);
  }

  async function save() {
    saving = true;
    saved = false;
    error = null;
    try {
      const accounts = rows
        .filter((r) => r.name.trim() !== "")
        .map((r) => ({
          name: r.name.trim(),
          type: r.type,
          opening_balance: Number(r.opening_balance) || 0,
        }));
      await ipc.runAppAction("finance", "save_accounts", { accounts });
      saved = true;
      setTimeout(() => (saved = false), 1500);
      onSaved?.();
    } catch (e) {
      error = `${e}`;
    } finally {
      saving = false;
    }
  }

  const selectClass =
    "h-7 rounded-md border border-border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring";

  onMount(load);
</script>

<div class="space-y-3 rounded-lg border border-border bg-card p-3">
  <div>
    <div class="font-display text-sm font-semibold">Accounts</div>
    <div class="text-xs text-muted-foreground">
      Your own accounts (checking, savings, card, cash) and their starting balances.
    </div>
  </div>

  {#if error}
    <div class="text-xs text-destructive">Couldn't update accounts: {error}. Try again.</div>
  {/if}

  {#if rows.length === 0}
    <div class="rounded-md border border-dashed border-border p-3 text-sm text-muted-foreground">
      No accounts yet.
    </div>
  {:else}
    <ul class="space-y-1.5">
      {#each rows as r, i (r._id)}
        <li class="flex items-center gap-2 rounded-md border border-border p-2 text-sm">
          <Input
            class="h-7 min-w-0 flex-1 text-xs"
            bind:value={r.name}
            placeholder="Account name"
            aria-label="Account name"
          />
          <select class={selectClass} bind:value={r.type} aria-label="Account type">
            {#each TYPES as t (t)}
              <option value={t}>{t}</option>
            {/each}
          </select>
          <input
            type="number"
            step="0.01"
            class="h-7 w-28 rounded-md border border-border bg-background px-2 text-right text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
            bind:value={r.opening_balance}
            placeholder="0.00"
            aria-label="Opening balance"
          />
          <button
            type="button"
            class="rounded-sm text-muted-foreground outline-none hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring"
            onclick={() => remove(i)}
            aria-label="Remove account"
          >
            <Trash2 class="size-4" />
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="flex items-center justify-between">
    <Button variant="outline" size="xs" onclick={add}>
      <Plus /> Add account
    </Button>
    <Button size="sm" onclick={save} disabled={saving}>
      <Check />
      {saved ? "Saved" : saving ? "Saving…" : "Save accounts"}
    </Button>
  </div>
</div>
