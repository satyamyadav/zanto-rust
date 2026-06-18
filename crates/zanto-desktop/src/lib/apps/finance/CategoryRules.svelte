<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Trash2, Plus } from "@lucide/svelte";

  type Rule = { id: number; merchant_contains: string; category: string };

  let { categories = [] }: { categories?: string[] } = $props();

  let rules = $state<Rule[]>([]);
  let error = $state<string | null>(null);

  let newMerchant = $state("");
  let newCategory = $state("");
  let saving = $state(false);

  async function load() {
    error = null;
    try {
      const res = await ipc.queryApp("finance", "category_rules");
      rules = (res?.rules ?? []) as Rule[];
    } catch (e) {
      error = `${e}`;
    }
  }

  async function add() {
    const merchant_contains = newMerchant.trim();
    if (!merchant_contains) return;
    saving = true;
    error = null;
    try {
      await ipc.runAppAction("finance", "save_category_rule", {
        merchant_contains,
        category: newCategory || categories[0] || "uncategorized",
      });
      newMerchant = "";
      await load();
    } catch (e) {
      error = `${e}`;
    } finally {
      saving = false;
    }
  }

  async function remove(id: number) {
    error = null;
    try {
      await ipc.runAppAction("finance", "delete_category_rule", { id });
      await load();
    } catch (e) {
      error = `${e}`;
    }
  }

  const selectClass =
    "h-7 rounded-md border border-border bg-background px-2 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring";

  onMount(load);

  $effect(() => {
    if (!newCategory && categories.length) newCategory = categories[0];
  });
</script>

<div class="space-y-3 rounded-lg border border-border bg-card p-3">
  <div>
    <div class="font-display text-sm font-semibold">Category rules</div>
    <div class="text-xs text-muted-foreground">
      Auto-categorize transactions whose merchant contains a phrase.
    </div>
  </div>

  {#if error}
    <div class="text-xs text-destructive">Couldn't update rules: {error}. Try again.</div>
  {/if}

  {#if rules.length === 0}
    <div class="rounded-md border border-dashed border-border p-3 text-sm text-muted-foreground">
      No rules yet.
    </div>
  {:else}
    <ul class="space-y-1.5">
      {#each rules as r (r.id)}
        <li class="flex items-center gap-2 rounded-md border border-border p-2 text-sm">
          <span class="min-w-0 flex-1 break-words">
            <span class="font-mono">{r.merchant_contains}</span>
            <span class="text-muted-foreground"> → </span>
            <span>{r.category}</span>
          </span>
          <button
            type="button"
            class="rounded-sm text-muted-foreground outline-none hover:text-destructive focus-visible:ring-2 focus-visible:ring-ring"
            onclick={() => remove(r.id)}
            aria-label="Delete rule"
          >
            <Trash2 class="size-4" />
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  <div class="flex flex-wrap items-center gap-2">
    <Input class="h-7 min-w-0 flex-1 text-xs" bind:value={newMerchant} placeholder="merchant contains…" />
    <select class={selectClass} bind:value={newCategory} aria-label="Category">
      {#each categories as c (c)}
        <option value={c}>{c}</option>
      {/each}
    </select>
    <Button size="xs" onclick={add} disabled={saving || !newMerchant.trim()}>
      <Plus /> Add
    </Button>
  </div>
</div>
