<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { interceptLinks } from "$lib/links.svelte";

  type PageSection = {
    heading?: string;
    markdown: string;
  };

  type PageData = {
    title?: string;
    sections: PageSection[];
  };

  let { data }: { data: PageData } = $props();

  const sections = $derived(
    (data.sections ?? []).map((s) => ({
      heading: s.heading,
      html: DOMPurify.sanitize(marked.parse(s.markdown ?? "") as string),
    })),
  );
</script>

<div>
  {#if data.title}
    <h1 class="mb-4 font-display text-2xl font-semibold">{data.title}</h1>
  {/if}
  {#each sections as section}
    <section class="mb-6">
      {#if section.heading}
        <h2 class="mb-2 font-display text-lg font-semibold">{section.heading}</h2>
      {/if}
      <!-- eslint-disable-next-line svelte/no-at-html-tags -->
      <div use:interceptLinks class="prose-zanto">{@html section.html}</div>
    </section>
  {/each}
</div>
