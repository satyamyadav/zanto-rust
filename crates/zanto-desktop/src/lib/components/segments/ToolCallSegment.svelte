<script lang="ts">
  // Minimal collapsible placeholder; C5 styles the tool-call view (status pill,
  // formatted args/result).
  let {
    name,
    args,
    output,
    ok,
  }: { name: string; args: any; output?: string; ok?: boolean } = $props();

  const pending = $derived(output === undefined);
  const status = $derived(pending ? "running" : ok ? "ok" : "error");
</script>

<details class="rounded-lg border border-border bg-muted/30 px-3 py-1.5 text-xs">
  <summary class="cursor-pointer select-none text-muted-foreground">
    <span class="font-mono text-foreground">{name}</span>
    <span class="ml-2">{status}</span>
  </summary>
  <pre class="mt-1 overflow-auto whitespace-pre-wrap text-muted-foreground">{JSON.stringify(args, null, 2)}</pre>
  {#if !pending}
    <pre class="mt-1 overflow-auto whitespace-pre-wrap text-muted-foreground">{output}</pre>
  {/if}
</details>
