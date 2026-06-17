<script lang="ts">
  import { ipc } from "$lib/ipc";
  import { Wallet, Check } from "@lucide/svelte";

  // Called after the profile is saved or the user skips, so the parent can
  // re-check setup state and swap this view out.
  let { onDone }: { onDone?: () => void } = $props();

  const DEFAULT_CATEGORIES = ["groceries", "dining", "transport", "utilities", "rent", "other"];

  let currency = $state("USD");
  let monthlyIncome = $state("");
  let categoriesText = $state(DEFAULT_CATEGORIES.join(", "));
  let saving = $state(false);
  let error = $state<string | null>(null);

  async function save() {
    saving = true;
    error = null;
    try {
      const categories = categoriesText
        .split(",")
        .map((c) => c.trim())
        .filter((c) => c.length > 0);
      const income = monthlyIncome.trim() === "" ? undefined : Number(monthlyIncome);
      await ipc.runAppAction("finance", "save_profile", {
        currency: currency.trim() || "USD",
        monthly_income: Number.isFinite(income) ? income : undefined,
        categories,
      });
      onDone?.();
    } catch (e) {
      error = `${e}`;
    } finally {
      saving = false;
    }
  }

  function skip() {
    onDone?.();
  }
</script>

<div class="flex h-full items-center justify-center">
  <div class="w-full max-w-sm space-y-4 rounded-lg border border-border p-5">
    <div class="flex items-center gap-2">
      <div class="rounded-full bg-muted p-2">
        <Wallet class="size-5 text-muted-foreground" />
      </div>
      <div>
        <div class="text-base font-semibold">Set up your finances</div>
        <div class="text-xs text-muted-foreground">A few details to get started. You can skip this.</div>
      </div>
    </div>

    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Currency</span>
      <input
        bind:value={currency}
        placeholder="USD"
        class="w-full rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
      />
    </label>

    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Monthly income (optional)</span>
      <input
        bind:value={monthlyIncome}
        type="number"
        inputmode="decimal"
        placeholder="e.g. 5000"
        class="w-full rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
      />
    </label>

    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Categories (comma-separated)</span>
      <input
        bind:value={categoriesText}
        class="w-full rounded-md border border-border bg-background px-2.5 py-1.5 text-sm outline-none focus:ring-1 focus:ring-ring"
      />
    </label>

    {#if error}
      <div class="text-xs text-destructive">Couldn't save: {error}</div>
    {/if}

    <div class="flex items-center justify-between pt-1">
      <button
        type="button"
        class="text-sm text-muted-foreground hover:text-foreground"
        onclick={skip}
        disabled={saving}
      >
        Skip for now
      </button>
      <button
        type="button"
        class="inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-50"
        onclick={save}
        disabled={saving}
      >
        <Check class="size-4" />
        {saving ? "Saving…" : "Save & continue"}
      </button>
    </div>
  </div>
</div>
