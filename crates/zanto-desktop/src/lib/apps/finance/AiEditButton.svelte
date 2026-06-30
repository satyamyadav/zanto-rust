<script lang="ts">
  import { send } from "$lib/stores/session.svelte";
  import Sparkles from "@lucide/svelte/icons/sparkles";

  let {
    prompt,
    size = "sm",
  }: {
    prompt: string;
    size?: "sm" | "md";
  } = $props();

  const dim = $derived(size === "md" ? "size-4" : "size-3.5");
</script>

<button
  type="button"
  title="Edit with AI"
  aria-label="Edit with AI"
  class="inline-flex shrink-0 items-center justify-center rounded-sm text-muted-foreground outline-none transition-colors hover:text-primary focus-visible:ring-2 focus-visible:ring-ring"
  onclick={() => send(prompt)}
>
  <Sparkles class={dim} />
</button>
