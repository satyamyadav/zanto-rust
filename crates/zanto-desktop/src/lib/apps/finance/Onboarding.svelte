<script lang="ts">
  import { ipc } from "$lib/ipc";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
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
  // Set once the user submits so validation messages only appear after a try.
  let touched = $state(false);

  const parsedCategories = $derived(
    categoriesText
      .split(",")
      .map((c) => c.trim())
      .filter((c) => c.length > 0),
  );
  const currencyError = $derived(currency.trim() === "" ? "Enter a currency code, e.g. USD." : null);
  const categoriesError = $derived(
    parsedCategories.length === 0 ? "Add at least one category." : null,
  );
  const valid = $derived(!currencyError && !categoriesError);

  async function save() {
    touched = true;
    if (!valid) return;
    saving = true;
    error = null;
    try {
      const income = monthlyIncome.trim() === "" ? undefined : Number(monthlyIncome);
      await ipc.runAppAction("finance", "save_profile", {
        currency: currency.trim(),
        monthly_income: Number.isFinite(income) ? income : undefined,
        categories: parsedCategories,
      });
      onDone?.();
    } catch (e) {
      error = `${e}`;
    } finally {
      saving = false;
    }
  }

  function skip() {
    if (!confirm("Skip setup? You can add your currency and categories later, but summaries will use defaults until then.")) {
      return;
    }
    onDone?.();
  }
</script>

<div class="flex h-full items-center justify-center">
  <div class="w-full max-w-sm space-y-4 rounded-lg border border-border bg-card p-5">
    <div class="flex items-center gap-2">
      <div class="rounded-full bg-accent p-2">
        <Wallet class="size-5 text-accent-foreground" />
      </div>
      <div>
        <div class="font-display text-base font-semibold">Set up your finances</div>
        <div class="text-xs text-muted-foreground">
          We'll use this to label amounts and group your spending.
        </div>
      </div>
    </div>

    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Currency</span>
      <Input
        bind:value={currency}
        placeholder="USD"
        class="font-mono"
        aria-invalid={touched && !!currencyError}
      />
      {#if touched && currencyError}
        <span class="text-xs text-destructive">{currencyError}</span>
      {/if}
    </label>

    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Monthly income (optional)</span>
      <Input
        bind:value={monthlyIncome}
        type="number"
        inputmode="decimal"
        placeholder="e.g. 5000"
        class="font-mono"
      />
    </label>

    <label class="block space-y-1">
      <span class="text-xs font-medium text-muted-foreground">Categories (comma-separated)</span>
      <Input
        bind:value={categoriesText}
        placeholder="groceries, dining, rent"
        aria-invalid={touched && !!categoriesError}
      />
      {#if touched && categoriesError}
        <span class="text-xs text-destructive">{categoriesError}</span>
      {/if}
    </label>

    {#if error}
      <div class="text-xs text-destructive">Couldn't save your profile: {error}. Try again.</div>
    {/if}

    <div class="flex items-center justify-between pt-1">
      <Button variant="ghost" size="sm" onclick={skip} disabled={saving}>Skip for now</Button>
      <Button onclick={save} disabled={saving}>
        <Check />
        {saving ? "Saving…" : "Save & continue"}
      </Button>
    </div>
  </div>
</div>
