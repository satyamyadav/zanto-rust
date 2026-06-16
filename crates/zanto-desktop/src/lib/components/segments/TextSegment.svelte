<script lang="ts">
  import Block from "$lib/Block.svelte";

  let { text }: { text: string } = $props();
</script>

<Block block={{ kind: "markdown", text }} />
