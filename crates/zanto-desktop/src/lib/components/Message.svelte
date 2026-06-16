<script lang="ts">
  import Block from "$lib/Block.svelte";
  import type { ChatEntry } from "$lib/stores/session.svelte";

  let { entry }: { entry: ChatEntry } = $props();
</script>

{#if entry.role === "user"}
  <div class="flex justify-end">
    <div class="max-w-[85%] rounded-2xl rounded-br-sm bg-secondary text-secondary-foreground px-4 py-2 text-sm">
      <Block block={entry.block} />
    </div>
  </div>
{:else}
  <div class="flex justify-start">
    <div class="max-w-[90%] text-sm leading-relaxed">
      <Block block={entry.block} />
    </div>
  </div>
{/if}
