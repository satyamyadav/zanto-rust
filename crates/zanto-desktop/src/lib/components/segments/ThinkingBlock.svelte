<script lang="ts">
  // ONE persistent "thinking/working" affordance hoisted to the top of an
  // assistant turn. It is a *presentation* over the turn's reasoning + tool
  // activity — no new data model. While the turn is live it shows a spinner + a
  // live activity label (the tail of the reasoning stream, else "Working…") and
  // stays expanded; when the turn finishes it collapses to a one-line "Thought…"
  // summary (never removed — content stays, expandable on click). Tool calls /
  // workflows / blocks / text render INLINE elsewhere, in document order.
  import { Loader, ChevronRight } from "@lucide/svelte";
  import { cn } from "$lib/utils";

  let { reasoning, stepCount, live }: { reasoning: string; stepCount: number; live: boolean } =
    $props();

  // Done-state summary: prefer the step count when the turn did any tool calls.
  const summary = $derived(
    stepCount > 0 ? `Thought for ${stepCount} step${stepCount === 1 ? "" : "s"}` : "Thought for a moment",
  );

  // Live label: the tail of the reasoning stream as a single line, else "Working…".
  const liveLabel = $derived.by(() => {
    const t = reasoning.trim();
    if (t.length === 0) return "Working…";
    const oneLine = t.replace(/\s+/g, " ");
    return oneLine.length > 80 ? `…${oneLine.slice(-80)}` : oneLine;
  });

  // While live, default to expanded so the user can watch the reasoning. When the
  // turn finishes, collapse. `open` is local state seeded from `live`; a $effect
  // collapses it once on the live→done transition without fighting manual toggles.
  let open = $state(live);
  let wasLive = live;
  $effect(() => {
    if (wasLive && !live) open = false;
    if (!wasLive && live) open = true;
    wasLive = live;
  });

  // Body content exists only when there is reasoning text.
  const hasReasoning = $derived(reasoning.trim().length > 0);
</script>

<div class="rounded-md border border-border bg-card text-xs">
  <button
    type="button"
    aria-expanded={open}
    onclick={() => (open = !open)}
    class="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
  >
    <ChevronRight
      size={12}
      class={cn(
        "shrink-0 text-muted-foreground transition-transform",
        open && hasReasoning && "rotate-90",
        !hasReasoning && "opacity-0",
      )}
    />
    {#if live}
      <Loader size={13} class="shrink-0 animate-spin text-primary agent-spine--live" />
      <span class="min-w-0 flex-1 truncate font-display text-muted-foreground">{liveLabel}</span>
    {:else}
      <span class="font-display font-medium text-foreground">{summary}</span>
    {/if}
  </button>

  {#if open && hasReasoning}
    <div class="border-t border-border px-3 py-2 whitespace-pre-wrap leading-relaxed text-muted-foreground">
      {reasoning}
    </div>
  {/if}
</div>
