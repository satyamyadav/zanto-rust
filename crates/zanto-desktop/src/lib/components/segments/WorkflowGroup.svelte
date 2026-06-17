<script lang="ts">
  import type { ChatSegment } from "$lib/stores/session.svelte";
  import ToolCallSegment from "./ToolCallSegment.svelte";
  import { Workflow, ChevronRight } from "@lucide/svelte";
  import { cn } from "$lib/utils";

  type ToolCallSegmentData = Extract<ChatSegment, { kind: "tool_call" }>;

  let { steps }: { steps: ToolCallSegmentData[] } = $props();

  const total = $derived(steps.length);
  const done = $derived(steps.filter((s) => s.output !== undefined).length);

  let open = $state(true);
</script>

<div class="rounded-lg border border-border bg-muted/20">
  <button
    type="button"
    aria-expanded={open}
    onclick={() => (open = !open)}
    class="flex w-full items-center gap-2 px-3 py-2 text-left text-xs"
  >
    <ChevronRight size={12} class={cn("shrink-0 text-muted-foreground transition-transform", open && "rotate-90")} />
    <Workflow size={13} class="shrink-0 text-muted-foreground" />
    <span class="font-medium text-foreground">Workflow ({total} steps)</span>
    <span class="ml-auto rounded-full bg-muted px-2 py-0.5 text-muted-foreground">{done}/{total} done</span>
  </button>

  {#if open}
    <div class="flex flex-col gap-2 border-t border-border px-2 pb-2 pt-2">
      {#each steps as step (step.id)}
        <ToolCallSegment name={step.name} args={step.args} output={step.output} ok={step.ok} />
      {/each}
    </div>
  {/if}
</div>
