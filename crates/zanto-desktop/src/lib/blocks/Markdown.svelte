<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { interceptLinks } from "$lib/links.svelte";

  type MarkdownData = {
    title?: string;
    content: string;
  };

  let { data }: { data: MarkdownData } = $props();

  const html = $derived(DOMPurify.sanitize(marked.parse(data.content ?? "") as string));
</script>

<div>
  {#if data.title}
    <div class="mb-2 text-sm font-medium text-foreground">{data.title}</div>
  {/if}
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  <div use:interceptLinks class="prose-zanto">{@html html}</div>
</div>
