<script lang="ts">
  import { ipc } from "$lib/ipc";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { ArrowRightLeft } from "@lucide/svelte";
  import { formatCurrency } from "./format";

  type Account = { name: string; type: string; balance: number };

  let {
    accounts = [],
    netWorth = 0,
    currency,
    onChanged,
  }: {
    accounts?: Account[];
    netWorth?: number;
    currency?: string;
    onChanged?: () => void;
  } = $props();

  let amount = $state("");
  let fromAccount = $state("");
  let toAccount = $state("");
  let transferring = $state(false);
  let error = $state<string | null>(null);

  // Seed the transfer selects from the first two account names once they arrive.
  $effect(() => {
    const names = accounts.map((a) => a.name);
    if (!fromAccount && names[0]) fromAccount = names[0];
    if (!toAccount && (names[1] ?? names[0])) toAccount = names[1] ?? names[0];
  });

  const amountValue = $derived(Number(amount));
  const canTransfer = $derived(
    Number.isFinite(amountValue) &&
      amountValue > 0 &&
      !!fromAccount &&
      !!toAccount &&
      fromAccount !== toAccount,
  );

  async function transfer() {
    if (!canTransfer) return;
    transferring = true;
    error = null;
    try {
      await ipc.runAppAction("finance", "add_transfer", {
        amount: Number(amount),
        from_account: fromAccount,
        to_account: toAccount,
      });
      amount = "";
      onChanged?.();
    } catch (e) {
      error = `${e}`;
    } finally {
      transferring = false;
    }
  }

  const selectClass =
    "h-9 rounded-md border border-border bg-background px-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring";
</script>

<div class="space-y-4">
  <div class="rounded-lg border border-border bg-card p-4">
    <div class="text-xs text-muted-foreground">Net worth</div>
    <div class="mt-1 font-display text-3xl font-semibold tabular-nums">
      {formatCurrency(netWorth, currency)}
    </div>
  </div>

  {#if accounts.length}
    <div class="grid grid-cols-2 gap-3">
      {#each accounts as a (a.name)}
        <div class="rounded-lg border border-border bg-card p-3">
          <div class="break-words text-sm font-medium">{a.name}</div>
          <div class="text-xs text-muted-foreground">{a.type}</div>
          <div
            class={[
              "mt-1 font-display text-xl font-semibold tabular-nums",
              a.balance < 0 ? "text-destructive" : "",
            ].join(" ")}
          >
            {formatCurrency(a.balance, currency)}
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <div class="rounded-lg border border-dashed border-border p-4 text-sm text-muted-foreground">
      No accounts yet. Add them from the dashboard's edit mode.
    </div>
  {/if}

  <div class="space-y-3 rounded-lg border border-border bg-card p-3">
    <div>
      <div class="font-display text-sm font-semibold">Transfer between accounts</div>
      <div class="text-xs text-muted-foreground">Move money from one account to another.</div>
    </div>

    {#if error}
      <div class="text-xs text-destructive">Couldn't transfer: {error}. Try again.</div>
    {/if}

    <div class="flex flex-wrap items-end gap-2">
      <Input
        type="number"
        min="0"
        step="0.01"
        class="h-9 w-32"
        bind:value={amount}
        placeholder="Amount"
        aria-label="Transfer amount"
      />
      <select class={selectClass} bind:value={fromAccount} aria-label="From account">
        {#each accounts as a (a.name)}
          <option value={a.name}>{a.name}</option>
        {/each}
      </select>
      <span class="text-muted-foreground"><ArrowRightLeft class="size-4" /></span>
      <select class={selectClass} bind:value={toAccount} aria-label="To account">
        {#each accounts as a (a.name)}
          <option value={a.name}>{a.name}</option>
        {/each}
      </select>
      <Button size="sm" onclick={transfer} disabled={!canTransfer || transferring}>
        {transferring ? "Transferring…" : "Transfer"}
      </Button>
    </div>
  </div>
</div>
