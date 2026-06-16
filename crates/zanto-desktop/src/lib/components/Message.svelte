<script lang="ts">
  import Block from "$lib/Block.svelte";
  import type { ChatEntry } from "$lib/stores/session.svelte";
  import TextSegment from "./segments/TextSegment.svelte";
  import ReasoningSegment from "./segments/ReasoningSegment.svelte";
  import ToolCallSegment from "./segments/ToolCallSegment.svelte";

  let { entry }: { entry: ChatEntry } = $props();
</script>

{#snippet segments()}
  {#each entry.segments as seg, i (i)}
    {#if seg.kind === "text"}
      <TextSegment text={seg.text} />
    {:else if seg.kind === "reasoning"}
      <ReasoningSegment text={seg.text} />
    {:else if seg.kind === "tool_call"}
      <ToolCallSegment name={seg.name} args={seg.args} output={seg.output} ok={seg.ok} />
    {:else if seg.kind === "block"}
      <Block block={seg.block} />
    {/if}
  {/each}
{/snippet}

{#if entry.role === "user"}
  <div class="flex justify-end">
    <div class="max-w-[85%] rounded-2xl rounded-br-sm bg-secondary text-secondary-foreground px-4 py-2 text-sm">
      {@render segments()}
    </div>
  </div>
{:else}
  <div class="flex justify-start">
    <div class="flex max-w-[90%] flex-col gap-2 text-sm leading-relaxed">
      {@render segments()}
    </div>
  </div>
{/if}
