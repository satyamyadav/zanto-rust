<script lang="ts">
  // Collapsible "Thinking" panel. Native <details> gives correct disclosure
  // semantics (default-collapsed, no flash, accessible) without coupling to the
  // global streaming flag — so historical reasoning never falsely animates.
  import { Brain, ChevronRight } from "@lucide/svelte";

  let { text }: { text: string } = $props();
</script>

<details
  class="group rounded-lg border border-border/60 bg-muted/30 text-xs text-muted-foreground"
>
  <summary
    class="flex cursor-pointer select-none items-center gap-1.5 px-3 py-1.5 list-none [&::-webkit-details-marker]:hidden hover:text-foreground"
  >
    <ChevronRight class="size-3 shrink-0 transition-transform duration-150 group-open:rotate-90" />
    <Brain class="size-3.5 shrink-0" />
    <span class="font-medium">Thinking</span>
  </summary>
  <div class="border-t border-border/40 px-3 py-2 whitespace-pre-wrap leading-relaxed">{text}</div>
</details>
