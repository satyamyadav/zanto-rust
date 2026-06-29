<script lang="ts">
  // ONE persistent "thinking/working" affordance hoisted to the top of an
  // assistant turn. It is a *presentation* over the turn's working text — the
  // model's reasoning tokens PLUS its intermediate prose narration (the text it
  // writes before/between tool calls) — no new data model. While the turn is
  // live it shows a spinner + a live activity label (the tail of that text, else
  // "Working…"); when finished it shows a one-line "Thought…" summary. It is
  // COLLAPSED by default (even while live) and expands on click to reveal the
  // full working text. The final answer, tool calls, blocks and errors render
  // INLINE elsewhere, in document order.
  import { Loader, ChevronRight } from "@lucide/svelte";
  import { cn } from "$lib/utils";

  let { text, stepCount, live }: { text: string; stepCount: number; live: boolean } = $props();

  // Done-state summary: prefer the step count when the turn did any tool calls.
  const summary = $derived(
    stepCount > 0 ? `Thought for ${stepCount} step${stepCount === 1 ? "" : "s"}` : "Thought for a moment",
  );

  // Live label: the tail of the working text as a single line, else "Working…".
  const liveLabel = $derived.by(() => {
    const t = text.trim();
    if (t.length === 0) return "Working…";
    const oneLine = t.replace(/\s+/g, " ");
    return oneLine.length > 80 ? `…${oneLine.slice(-80)}` : oneLine;
  });

  // Collapsed by default — even while live. Click to expand and read the full
  // working text. Local state; never auto-opened.
  let open = $state(false);

  // Body content (and the chevron affordance) exist only when there is text.
  const hasText = $derived(text.trim().length > 0);
</script>

<div class="text-xs">
  <button
    type="button"
    aria-expanded={open}
    disabled={!hasText}
    onclick={() => hasText && (open = !open)}
    class="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:cursor-default disabled:hover:bg-transparent"
  >
    <ChevronRight
      size={12}
      class={cn(
        "shrink-0 text-muted-foreground transition-transform",
        open && hasText && "rotate-90",
        !hasText && "opacity-0",
      )}
    />
    {#if live}
      <Loader size={13} class="shrink-0 animate-spin text-primary agent-spine--live" />
      <span class="min-w-0 flex-1 truncate font-display text-muted-foreground">{liveLabel}</span>
    {:else}
      <span class="font-display font-medium text-foreground">{summary}</span>
    {/if}
  </button>

  {#if open && hasText}
    <!-- Indented under the chevron with a faint left guide — hierarchy by
         indentation, not a boxed card. -->
    <div class="ml-3 border-l border-border/50 pl-3 py-1 whitespace-pre-wrap leading-relaxed text-muted-foreground">
      {text}
    </div>
  {/if}
</div>
