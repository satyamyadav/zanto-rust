<script lang="ts">
  import Message from "./Message.svelte";
  import Composer from "./Composer.svelte";
  import HitlForm from "./HitlForm.svelte";
  import { sessionStore } from "$lib/stores/session.svelte";

  let scroller: HTMLDivElement;

  // Pin to bottom as new entries arrive.
  $effect(() => {
    sessionStore.convo.length;
    sessionStore.busy;
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  });
</script>

<div class="flex h-full flex-col min-w-0">
  <div bind:this={scroller} class="flex-1 overflow-auto p-4 space-y-3">
    {#each sessionStore.convo as entry, i (i)}
      <Message {entry} />
    {/each}
    {#if sessionStore.busy}
      <div class="text-sm text-muted-foreground">…thinking</div>
    {/if}
    {#if sessionStore.convo.length === 0 && !sessionStore.busy}
      <div class="text-sm text-muted-foreground">Start a conversation.</div>
    {/if}
  </div>
  <div class="relative">
    <HitlForm />
    <Composer />
  </div>
</div>
