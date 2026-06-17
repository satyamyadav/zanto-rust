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

  let open = $state(true);
</script>

<div class="rounded-md border border-border bg-card">
  <button
    type="button"
    aria-expanded={open}
    onclick={() => (open = !open)}
    class="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
  >
    <ChevronRight size={12} class={cn("shrink-0 text-muted-foreground transition-transform", open && "rotate-90")} />
    <Workflow size={13} class="shrink-0 text-muted-foreground" />
    <span class="font-display font-medium text-foreground">Workflow ({total} steps)</span>
    {#if pill === "error"}
      <span class="ml-auto rounded-full bg-destructive px-2 py-0.5 font-mono text-destructive-foreground">{done}/{total} done</span>
    {:else if pill === "done"}
      <span class="ml-auto rounded-full bg-success px-2 py-0.5 font-mono text-success-foreground">{done}/{total} done</span>
    {:else}
      <span class="ml-auto rounded-full bg-muted px-2 py-0.5 font-mono text-muted-foreground">{done}/{total} done</span>
    {/if}
  </button>

  {#if open}
    <div class="flex flex-col gap-2 border-t border-border px-2 pb-2 pt-2">
      {#each steps as step (step.id)}
        <ToolCallSegment name={step.name} args={step.args} output={step.output} ok={step.ok} />
      {/each}
    </div>
  {/if}
</div>
