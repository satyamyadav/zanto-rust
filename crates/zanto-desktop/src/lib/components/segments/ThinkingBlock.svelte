<script lang="ts">
  // Always-on per-turn "Working/Thinking" affordance. Wraps an assistant turn's
  // process segments (reasoning + the tool_call/workflow cluster) under one
  // collapsible header. It is a *presentation* over existing segments — no new
  // data model. While the turn is live it shows a pulse + a live activity label
  // and stays expanded so the user sees steps; when done it collapses to a
  // one-line "Thought for N steps" summary, expandable on click.
  import type { ChatSegment } from "$lib/stores/session.svelte";
  import ReasoningSegment from "./ReasoningSegment.svelte";
  import ToolCallSegment from "./ToolCallSegment.svelte";
  import WorkflowGroup from "./WorkflowGroup.svelte";
  import { Loader, ChevronRight } from "@lucide/svelte";
  import { cn } from "$lib/utils";

  type ToolCallSegmentData = Extract<ChatSegment, { kind: "tool_call" }>;
  // A process item is either a reasoning segment, a lone tool_call, or a
  // coalesced workflow run (mirrors Message.svelte's RenderItem for process).
  export type ProcessItem =
    | { kind: "reasoning"; seg: Extract<ChatSegment, { kind: "reasoning" }> }
    | { kind: "tool"; seg: ToolCallSegmentData }
    | { kind: "workflow"; steps: ToolCallSegmentData[] };

  let { items, live }: { items: ProcessItem[]; live: boolean } = $props();

  // Step count = number of tool_call invocations (lone tools + every step of
  // each workflow). Reasoning is not a "step".
  const stepCount = $derived(
    items.reduce(
      (n, it) => n + (it.kind === "tool" ? 1 : it.kind === "workflow" ? it.steps.length : 0),
      0,
    ),
  );

  // Done-state summary: "Thought for N steps" when any tools ran; if none ran
  // but the model reasoned, "Thought for a moment".
  const summary = $derived(
    stepCount > 0
      ? `Thought for ${stepCount} step${stepCount === 1 ? "" : "s"}`
      : "Thought for a moment",
  );

  // Live label, in priority order: latest reasoning text → active tool name →
  // "Thinking…". A tool is "active" when it has been issued but has no output.
  function activeToolName(): string | null {
    for (let i = items.length - 1; i >= 0; i--) {
      const it = items[i];
      if (it.kind === "tool" && it.seg.output === undefined) return it.seg.name;
      if (it.kind === "workflow") {
        const pending = it.steps.find((s) => s.output === undefined);
        if (pending) return pending.name;
      }
    }
    return null;
  }

  function latestReasoning(): string | null {
    for (let i = items.length - 1; i >= 0; i--) {
      const it = items[i];
      if (it.kind === "reasoning") {
        const t = it.seg.text.trim();
        if (t.length > 0) return t;
      }
    }
    return null;
  }

  const liveLabel = $derived.by(() => {
    const reasoning = latestReasoning();
    if (reasoning) {
      // Show the tail of the reasoning stream as a single line.
      const oneLine = reasoning.replace(/\s+/g, " ");
      return oneLine.length > 80 ? `…${oneLine.slice(-80)}` : oneLine;
    }
    const tool = activeToolName();
    if (tool) return `Running ${tool}…`;
    return "Thinking…";
  });

  // While live, default to expanded so the user can watch steps. When the turn
  // finishes, collapse. `open` is local state seeded from `live`; a $effect
  // collapses it once on the live→done transition without fighting the user's
  // manual toggles afterwards.
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
      {#if stepCount > 0}
        <span class="ml-auto rounded-full bg-muted px-2 py-0.5 font-mono text-muted-foreground">
          {stepCount} step{stepCount === 1 ? "" : "s"}
        </span>
      {/if}
    {/if}
  </button>

  {#if open}
    <ol class="flex flex-col border-t border-border px-2 py-2">
      {#each items as item, si (si)}
        {@const isActiveStep = live && si === items.length - 1}
        <li class="relative flex gap-3 pb-2 last:pb-0">
          <!-- Rail + node column, mirroring the agent spine. -->
          <div class="relative flex w-3 shrink-0 justify-center" aria-hidden="true">
            <span
              class={cn(
                "absolute inset-y-0 w-px",
                isActiveStep ? "bg-primary/60 agent-spine--live" : "bg-border",
              )}
            ></span>
            <span
              class={cn(
                "relative z-10 mt-1.5 size-2 rounded-full ring-2 ring-background",
                isActiveStep ? "bg-primary agent-spine--live" : "bg-border",
              )}
            ></span>
          </div>
          <div class="min-w-0 flex-1">
            {#if item.kind === "reasoning"}
              <ReasoningSegment text={item.seg.text} />
            {:else if item.kind === "tool"}
              <ToolCallSegment
                name={item.seg.name}
                args={item.seg.args}
                output={item.seg.output}
                ok={item.seg.ok}
              />
            {:else}
              <WorkflowGroup steps={item.steps} />
            {/if}
          </div>
        </li>
      {/each}
    </ol>
  {/if}
</div>
