<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { Button } from "$lib/components/ui/button";
  import { Check } from "@lucide/svelte";

  type Budget = { category: string; limit: number };

  let { categories = [], onSaved }: { categories?: string[]; onSaved?: () => void } = $props();

  // Working map of category → limit input string, seeded from saved budgets.
  let limits = $state<Record<string, string>>({});
  let error = $state<string | null>(null);
  let saving = $state(false);
  let saved = $state(false);

  async function load() {
    error = null;
    try {
      const res = await ipc.queryApp("finance", "budgets");
      const budgets = (res?.budgets ?? []) as Budget[];
      const next: Record<string, string> = {};
      for (const b of budgets) next[b.category] = `${b.limit}`;
      limits = next;
    } catch (e) {
      error = `${e}`;
    }
  }

  async function save() {
    saving = true;
    saved = false;
    error = null;
    try {
      const budgets: Budget[] = [];
      for (const [category, raw] of Object.entries(limits)) {
        const limit = Number(raw);
        if (Number.isFinite(limit) && limit > 0) budgets.push({ category, limit });
      }
      await ipc.runAppAction("finance", "save_budgets", { budgets });
      saved = true;
      setTimeout(() => (saved = false), 1500);
      onSaved?.();
    } catch (e) {
      error = `${e}`;
    } finally {
      saving = false;
    }
  }

  onMount(load);
</script>

<div class="space-y-3 rounded-lg border border-border bg-card p-3">
  <div>
    <div class="font-display text-sm font-semibold">Monthly budgets</div>
    <div class="text-xs text-muted-foreground">
      Set a spending limit per category. Leave blank for no limit.
    </div>
  </div>

  {#if error}
    <div class="text-xs text-destructive">Couldn't update budgets: {error}. Try again.</div>
  {/if}

  {#if categories.length === 0}
    <div class="rounded-md border border-dashed border-border p-3 text-sm text-muted-foreground">
      No categories yet.
    </div>
  {:else}
    <ul class="space-y-1.5">
      {#each categories as c (c)}
        <li class="flex items-center gap-2 rounded-md border border-border p-2 text-sm">
          <span class="min-w-0 flex-1 break-words">{c}</span>
          <input
            type="number"
            min="0"
            step="0.01"
            class="h-7 w-28 rounded-md border border-border bg-background px-2 text-right text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
            bind:value={limits[c]}
            placeholder="no limit"
            aria-label={`Budget for ${c}`}
          />
        </li>
      {/each}
    </ul>
  {/if}

  <div class="flex items-center justify-end">
    <Button size="sm" onclick={save} disabled={saving}>
      <Check />
      {saved ? "Saved" : saving ? "Saving…" : "Save budgets"}
    </Button>
  </div>
</div>
