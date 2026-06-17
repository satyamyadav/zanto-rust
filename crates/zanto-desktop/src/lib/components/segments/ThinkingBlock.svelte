<script lang="ts">
  // Reasoning-only "Thinking" affordance for an assistant turn. Wraps a single
  // run of reasoning text under one collapsible header. It is a *presentation*
  // over the reasoning segment — no new data model. While the turn is live it
  // shows a pulse + a live activity label (the tail of the reasoning stream) and
  // stays expanded; when done it collapses to a one-line "Thought…" summary,
  // expandable on click. Tool calls / workflows / blocks render inline elsewhere.
  import { Loader, ChevronRight } from "@lucide/svelte";
  import { cn } from "$lib/utils";

  let { text, live }: { text: string; live: boolean } = $props();

  // Done-state summary.
  const summary = "Thought for a moment";

  // Live label: the tail of the reasoning stream as a single line, else "Thinking…".
  const liveLabel = $derived.by(() => {
    const t = text.trim();
    if (t.length === 0) return "Thinking…";
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
      class={cn("shrink-0 text-muted-foreground transition-transform", open && "rotate-90")}
    />
    {#if live}
      <Loader size={13} class="shrink-0 animate-spin text-primary agent-spine--live" />
      <span class="min-w-0 flex-1 truncate font-display text-muted-foreground">{liveLabel}</span>
    {:else}
      <span class="font-display font-medium text-foreground">{summary}</span>
    {/if}
  </button>

  {#if open}
    <div class="border-t border-border px-3 py-2 whitespace-pre-wrap leading-relaxed text-muted-foreground">
      {text}
    </div>
  {/if}
</div>
