<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Check, Plus, Trash2 } from "@lucide/svelte";

  // `_id` is a stable per-row client key so bound inputs don't re-associate to the
  // wrong row after a remove (B4-2). It is never sent to the backend.
  type GoalRow = {
    _id: number;
    name: string;
    kind: "savings" | "debt";
    account: string;
    target: number | string;
    target_date: string;
  };

  let rowSeq = 0;
  const nextId = () => ++rowSeq;

  let { accounts, onSaved }: { accounts?: string[]; onSaved?: () => void } = $props();

  let rows = $state<GoalRow[]>([]);
  let error = $state<string | null>(null);
  let saving = $state(false);
  let saved = $state(false);

  async function load() {
    error = null;
    try {
      const res: { goals?: GoalRow[] } = await ipc.queryApp("finance", "goals");
      rows = (res?.goals ?? []).map((g) => ({
        _id: nextId(),
        name: g.name,
        kind: g.kind,
        account: g.account,
        target: g.target,
        target_date: g.target_date ?? "",
      }));
    } catch (e) {
      error = `${e}`;
    }
  }

  function add() {
    rows = [...rows, { _id: nextId(), name: "", kind: "savings", account: accounts?.[0] ?? "", target: "", target_date: "" }];
  }

  function remove(i: number) {
    rows = rows.filter((_, j) => j !== i);
  }

  async function save() {
    saving = true;
    saved = false;
    error = null;
    try {
      const goals = rows
        .filter((r) => r.name.trim() !== "")
        .map((r) => ({
          name: r.name.trim(),
          kind: r.kind,
          account: r.account,
          target: Number(r.target) || 0,
          target_date: r.target_date.trim(),
        }));
      await ipc.runAppAction("finance", "save_goals", { goals });
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
    <div class="font-display text-sm font-semibold">Goals</div>
    <div class="text-xs text-muted-foreground">
      Track a savings target or debt payoff against one of your accounts.
    </div>
  </div>

  {#if error}
    <div class="text-xs text-destructive">Couldn't update goals: {error}. Try again.</div>
  {/if}

  {#if rows.length === 0}
    <div class="rounded-md border border-dashed border-border p-3 text-sm text-muted-foreground">
      No goals yet.
    </div>
  {:else}
    <ul class="space-y-1.5">
      {#each rows as r, i (r._id)}
        <li class="flex items-center gap-2 rounded-md border border-border p-2 text-sm">
          <Input
            class="h-7 min-w-0 flex-1 text-xs"
            bind:value={r.name}
            placeholder="Goal name"
            aria-label="Goal name"
          />
          <select class={selectClass} bind:value={r.kind} aria-label="Goal kind">
            <option value="savings">savings</option>
            <option value="debt">debt</option>
          </select>
          <select class={selectClass} bind:value={r.account} aria-label="Account">
            {#each accounts ?? [] as a (a)}
              <option value={a}>{a}</option>
            {/each}
          </select>
          <input
            type="number"
            step="0.01"
            class="h-7 w-28 rounded-md border border-border bg-background px-2 text-right text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
            bind:value={r.target}
            placeholder="0.00"
            aria-label="Target amount"
          />
          <input
            type="text"
            class="h-7 w-24 rounded-md border border-border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
            bind:value={r.target_date}
            placeholder="YYYY-MM"
            aria-label="Target date"
          />
          <button
            type="button"
            class="rounded-sm text-muted-foreground outline-none hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring"
            onclick={() => remove(i)}
            aria-label="Remove goal"
          >
            <Trash2 class="size-4" />
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="flex items-center justify-between">
    <Button variant="outline" size="xs" onclick={add}>
      <Plus /> Add goal
    </Button>
    <Button size="sm" onclick={save} disabled={saving}>
      <Check />
      {saved ? "Saved" : saving ? "Saving…" : "Save goals"}
    </Button>
  </div>
</div>
