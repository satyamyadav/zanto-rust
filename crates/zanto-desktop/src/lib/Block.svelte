<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import type { ChatBlock } from "./ipc";
  import { componentRegistry } from "./registry";
  import { validateArtifact } from "./stores/artifacts.svelte";
  import Json from "./blocks/Json.svelte";

  let { block }: { block: ChatBlock } = $props();

  const html = $derived(
    block.kind === "markdown" ? DOMPurify.sanitize(marked.parse(block.text) as string) : ""
  );
  const Comp = $derived(block.kind === "component" ? componentRegistry[block.component_id] : undefined);
  const valid = $derived(
    block.kind === "component" ? validateArtifact(block.component_id, block.data) : true
  );
</script>

{#if block.kind === "component"}
  {#if Comp && valid}
    <Comp data={block.data} />
  {:else}
    <Json data={block.data} />
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
