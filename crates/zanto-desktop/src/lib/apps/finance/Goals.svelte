<script lang="ts">
  import { formatCurrency } from "./format";

  type GoalStatus = {
    name: string;
    kind: "savings" | "debt";
    account: string;
    target: number;
    current?: number;
    owed?: number;
    progress: number;
    remaining?: number;
    complete: boolean;
  };

  let { goalStatus, currency }: { goalStatus?: GoalStatus[]; currency?: string } = $props();

  const goals = $derived<GoalStatus[]>(goalStatus ?? []);

  function money(v: number | undefined): string {
    return formatCurrency(v, currency);
  }
</script>

{#if goals.length}
  <div class="space-y-3">
    {#each goals as g (g.name)}
      <div class="space-y-1">
        <div class="flex items-center justify-between text-sm">
          <span class="min-w-0 break-words">{g.name}</span>
          <span class="font-mono tabular-nums text-muted-foreground">
            {#if g.kind === "savings"}
              {money(g.current)} / {money(g.target)}
            {:else}
              {money(g.owed)} left
            {/if}
          </span>
        </div>
        <div class="h-2 overflow-hidden rounded-full bg-muted">
          <div
            class={["h-full rounded-full", g.complete ? "bg-success" : "bg-primary"].join(" ")}
            style={`width: ${Math.min(1, Math.max(0, g.progress)) * 100}%`}
          ></div>
        </div>
      </div>
    {/each}
  </div>
{:else}
  <div class="rounded-md border border-dashed border-border p-3 text-sm text-muted-foreground">
    No goals yet — add one in Edit mode.
  </div>
{/if}
