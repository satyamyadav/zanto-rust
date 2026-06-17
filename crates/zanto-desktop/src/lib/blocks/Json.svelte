<script lang="ts">
  let { data }: { data: any } = $props();
  const text = $derived(JSON.stringify(data?.value ?? data, null, 2));

  let copied = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy() {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      clearTimeout(timer);
      timer = setTimeout(() => (copied = false), 1500);
    } catch {
      copied = false;
    }
  }
</script>

<div class="relative">
  <button
    type="button"
    onclick={copy}
    class="absolute right-1.5 top-1.5 rounded-sm border border-border bg-background px-1.5 py-0.5 text-xs text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
  >
    {copied ? "Copied" : "Copy"}
  </button>
  <pre
    class="overflow-x-auto rounded-md border border-border bg-muted p-2 pr-16 font-mono text-xs text-foreground">{text}</pre>
</div>
