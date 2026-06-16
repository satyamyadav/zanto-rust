<script lang="ts">
  import type { ChatBlock } from "./ipc";
  import { componentRegistry } from "./registry";

  let { block }: { block: ChatBlock } = $props();
</script>

{#if block.kind === "component"}
  {@const Comp = componentRegistry[block.component_id]}
  {#if Comp}
    <Comp data={block.data} />
  {:else}
    <pre class="text-xs bg-gray-100 p-2 rounded overflow-auto">{JSON.stringify(block.data, null, 2)}</pre>
  {/if}
{:else}
  <div class="whitespace-pre-wrap">{block.text}</div>
{/if}
