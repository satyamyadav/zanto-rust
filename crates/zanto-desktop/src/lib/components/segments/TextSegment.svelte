<script lang="ts">
  import Block from "$lib/Block.svelte";

  let { text }: { text: string } = $props();
</script>

<!-- Wrapper keeps long content (code, tables, urls) from overflowing the bubble
     and tightens spacing on the markdown Block, while preserving its renderer. -->
<div class="min-w-0 text-pretty break-words [&_.prose-zanto>:first-child]:mt-0 [&_.prose-zanto>:last-child]:mb-0">
  <Block block={{ kind: "markdown", text }} />
</div>
