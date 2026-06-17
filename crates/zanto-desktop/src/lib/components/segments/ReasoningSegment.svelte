<script lang="ts">
  // Collapsible "Thinking" panel. Native <details> gives correct disclosure
  // semantics (default-collapsed, no flash, accessible) without coupling to the
  // global streaming flag — so historical reasoning never falsely animates.
  import { Brain, ChevronRight } from "@lucide/svelte";

  let { text }: { text: string } = $props();
</script>

<details class="group rounded-md border border-border bg-card text-xs text-muted-foreground">
  <summary
    class="flex cursor-pointer select-none items-center gap-1.5 rounded-md px-3 py-1.5 list-none [&::-webkit-details-marker]:hidden hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
  >
    <ChevronRight class="size-3 shrink-0 transition-transform duration-150 group-open:rotate-90" />
    <Brain class="size-3.5 shrink-0" />
    <span class="font-display font-medium">Thinking</span>
  </summary>
  <div class="border-t border-border px-3 py-2 whitespace-pre-wrap leading-relaxed">{text}</div>
</details>
