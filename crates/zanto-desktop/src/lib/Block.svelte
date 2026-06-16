<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import type { ChatBlock } from "./ipc";
  import { componentRegistry } from "./registry";

  let { block }: { block: ChatBlock } = $props();

  const html = $derived(
    block.kind === "markdown" ? DOMPurify.sanitize(marked.parse(block.text) as string) : ""
  );
</script>

{#if block.kind === "component"}
  {@const Comp = componentRegistry[block.component_id]}
  {#if Comp}
    <Comp data={block.data} />
  {:else}
    <pre class="text-xs bg-muted p-2 rounded overflow-auto">{JSON.stringify(block.data, null, 2)}</pre>
  {/if}
{:else}
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  <div class="prose-zanto">{@html html}</div>
{/if}

<style>
  .prose-zanto :global(p) { margin: 0.25rem 0; }
  .prose-zanto :global(ul) { margin: 0.25rem 0; padding-left: 1.25rem; list-style: disc; }
  .prose-zanto :global(ol) { margin: 0.25rem 0; padding-left: 1.25rem; list-style: decimal; }
  .prose-zanto :global(code) { font-family: ui-monospace, monospace; font-size: 0.85em; }
  .prose-zanto :global(pre) { background: var(--muted); padding: 0.5rem; border-radius: 0.4rem; overflow: auto; }
  .prose-zanto :global(table) { border-collapse: collapse; }
  .prose-zanto :global(th), .prose-zanto :global(td) { border: 1px solid var(--border); padding: 0.2rem 0.5rem; }
</style>
