<script lang="ts">
  import { formatCurrency } from "./format";

  type Status = { category: string; limit: number; spent: number; pct: number; over: boolean };

  let { status, currency }: { status: Status[]; currency?: string } = $props();

  function fillClass(s: Status): string {
    if (s.over) return "bg-destructive";
    if (s.pct >= 0.8) return "bg-warning";
    return "bg-success";
  }
</script>

{#if status.length}
  <div class="space-y-3">
    {#each status as s (s.category)}
      <div class="space-y-1">
        <div class="flex items-center justify-between text-sm">
          <span class="min-w-0 break-words">{s.category}</span>
          <span class="font-mono tabular-nums text-muted-foreground">
            {formatCurrency(s.spent, currency)} / {formatCurrency(s.limit, currency)}
          </span>
        </div>
        <div class="h-2 overflow-hidden rounded-full bg-muted">
          <div
            class={["h-full rounded-full", fillClass(s)].join(" ")}
            style={`width: ${Math.min(1, s.pct) * 100}%`}
          ></div>
        </div>
      </div>
    {/each}
  </div>
{/if}
