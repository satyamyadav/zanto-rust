<script lang="ts">
  // Minimal collapsible placeholder; C4 styles the "Thinking" view.
  let { text }: { text: string } = $props();
</script>

<details class="rounded-lg border border-border bg-muted/30 px-3 py-1.5 text-xs text-muted-foreground">
  <summary class="cursor-pointer select-none">Thinking</summary>
  <div class="mt-1 whitespace-pre-wrap">{text}</div>
</details>
