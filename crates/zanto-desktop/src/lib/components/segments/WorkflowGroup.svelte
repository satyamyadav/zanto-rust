<script lang="ts">
  import type { ChatSegment } from "$lib/stores/session.svelte";
  import ToolCallSegment from "./ToolCallSegment.svelte";
  import { Workflow, ChevronRight } from "@lucide/svelte";
  import { cn } from "$lib/utils";

  type ToolCallSegmentData = Extract<ChatSegment, { kind: "tool_call" }>;

  let { steps }: { steps: ToolCallSegmentData[] } = $props();

  const total = $derived(steps.length);
  const done = $derived(steps.filter((s) => s.output !== undefined).length);
  const failed = $derived(steps.some((s) => s.output !== undefined && s.ok === false));
  // Pill state: destructive if any step failed, success once every step is in,
  // otherwise a quiet neutral "in progress".
  const pill = $derived(
    failed ? "error" : done === total ? "done" : "running",
  );

  // Collapsed by default — click the header to reveal the steps.
  let open = $state(false);
</script>

<div>
  <button
    type="button"
    aria-expanded={open}
    onclick={() => (open = !open)}
    class="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left text-xs transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
  >
    <ChevronRight size={12} class={cn("shrink-0 text-muted-foreground transition-transform", open && "rotate-90")} />
    <Workflow size={13} class="shrink-0 text-muted-foreground" />
    <span class="font-display font-medium text-foreground">Workflow ({total} steps)</span>
    {#if pill === "error"}
      <span class="ml-auto rounded-full bg-destructive-soft px-2 py-0.5 font-mono text-destructive-soft-foreground">{done}/{total} done</span>
    {:else if pill === "done"}
      <span class="ml-auto rounded-full bg-success-soft px-2 py-0.5 font-mono text-success-soft-foreground">{done}/{total} done</span>
    {:else}
      <span class="ml-auto rounded-full bg-warning-soft px-2 py-0.5 font-mono text-warning-soft-foreground">{done}/{total} done</span>
    {/if}
  </button>

  {#if open}
    <!-- Steps indented under the workflow row with a faint left guide — they read
         as children of the workflow, not a boxed list. Each step is itself
         borderless, so no card-in-card. -->
    <div class="ml-3 flex flex-col gap-0.5 border-l border-border/50 pl-2 pt-0.5">
      {#each steps as step (step.id)}
        <ToolCallSegment name={step.name} args={step.args} output={step.output} ok={step.ok} />
      {/each}
    </div>
  {/if}
</div>
