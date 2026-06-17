<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import type { ChatBlock } from "./ipc";
  import { componentRegistry } from "./registry";
  import { validateArtifact } from "./stores/artifacts.svelte";
  import { interceptLinks } from "./links.svelte";
  import Json from "./blocks/Json.svelte";

  let { block }: { block: ChatBlock } = $props();

  const html = $derived(
    block.kind === "markdown" ? DOMPurify.sanitize(marked.parse(block.text) as string) : ""
  );
  const Comp = $derived(block.kind === "component" ? componentRegistry[block.component_id] : undefined);
  const valid = $derived(
    block.kind === "component" ? validateArtifact(block.component_id, block.data) : true
  );
  const unknown = $derived(block.kind === "component" && !Comp);
</script>

{#if block.kind === "component"}
  {#if Comp && valid}
    <Comp data={block.data} />
  {:else}
    {#if unknown}
      <div class="mb-1 font-mono text-xs text-muted-foreground">
        Unknown artifact: {block.component_id}
      </div>
    {/if}
    <Json data={block.data} />
  {/if}
{:else}
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  <div use:interceptLinks class="prose-zanto">{@html html}</div>
{/if}
