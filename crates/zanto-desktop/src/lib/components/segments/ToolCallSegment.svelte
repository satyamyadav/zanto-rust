<script lang="ts">
  import { CheckCircle, XCircle, Loader, ChevronRight } from "@lucide/svelte";
  import { cn } from "$lib/utils";

  let {
    name,
    args,
    output,
    ok,
  }: { name: string; args: any; output?: string; ok?: boolean } = $props();

  const pending = $derived(output === undefined);
  const status = $derived(pending ? "running" : ok ? "ok" : "error");

  const argsJson = $derived(() => {
    try {
      return JSON.stringify(args, null, 2);
    } catch {
      return "(unserializable args)";
    }
  });

  let argsOpen = $state(true);
  let outputOpen = $state(false);
</script>

{#snippet section(label: string, open: boolean, toggle: () => void, content: string)}
  <div class="border-t border-border">
    <button
      type="button"
      aria-expanded={open}
      onclick={toggle}
      class="flex w-full items-center gap-1 px-3 py-1 text-left text-muted-foreground hover:text-foreground"
    >
      <ChevronRight size={12} class={cn("transition-transform", open && "rotate-90")} />
      {label}
    </button>
    {#if open}
      <pre class="overflow-auto whitespace-pre-wrap px-3 pb-2 font-mono text-muted-foreground">{content}</pre>
    {/if}
  </div>
{/snippet}

<div class="rounded-lg border border-border bg-muted/30 text-xs">
  <!-- Header row: name + status pill -->
  <div class="flex items-center gap-2 px-3 py-2">
    <span class="font-mono font-medium text-foreground">{name}</span>

    {#if status === "running"}
      <span class="flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-muted-foreground">
        <Loader size={10} class="animate-spin" />
        running
      </span>
    {:else if status === "ok"}
      <span class="flex items-center gap-1 rounded-full bg-green-500/15 px-2 py-0.5 text-green-600 dark:text-green-400">
        <CheckCircle size={10} />
        done
      </span>
    {:else}
      <span class="flex items-center gap-1 rounded-full bg-destructive/15 px-2 py-0.5 text-destructive">
        <XCircle size={10} />
        error
      </span>
    {/if}
  </div>

  {@render section("args", argsOpen, () => (argsOpen = !argsOpen), argsJson())}

  {#if !pending}
    {@render section("output", outputOpen, () => (outputOpen = !outputOpen), output ?? "")}
  {/if}
</div>
