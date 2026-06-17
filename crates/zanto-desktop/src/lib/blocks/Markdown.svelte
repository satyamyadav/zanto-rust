<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";

  type MarkdownData = {
    title?: string;
    content: string;
  };

  let { data }: { data: MarkdownData } = $props();

  const html = $derived(DOMPurify.sanitize(marked.parse(data.content ?? "") as string));
</script>

<div>
  {#if data.title}
    <div class="text-sm font-medium mb-2">{data.title}</div>
  {/if}
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  <div class="prose-zanto">{@html html}</div>
</div>

<style>
  .prose-zanto :global(p) { margin: 0.25rem 0; }
  .prose-zanto :global(ul) { margin: 0.25rem 0; padding-left: 1.25rem; list-style: disc; }
  .prose-zanto :global(ol) { margin: 0.25rem 0; padding-left: 1.25rem; list-style: decimal; }
  .prose-zanto :global(h1), .prose-zanto :global(h2), .prose-zanto :global(h3) { font-weight: 600; margin: 0.5rem 0 0.25rem; }
  .prose-zanto :global(code) { font-family: ui-monospace, monospace; font-size: 0.85em; }
  .prose-zanto :global(pre) { background: var(--muted); padding: 0.5rem; border-radius: 0.4rem; overflow: auto; }
  .prose-zanto :global(table) { border-collapse: collapse; }
  .prose-zanto :global(th), .prose-zanto :global(td) { border: 1px solid var(--border); padding: 0.2rem 0.5rem; }
</style>
