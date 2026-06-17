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

  const argsJson = $derived.by(() => {
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
      class="flex w-full items-center gap-1 rounded-sm px-3 py-1 text-left font-mono text-xs text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <ChevronRight size={12} class={cn("transition-transform", open && "rotate-90")} />
      {label}
    </button>
    {#if open}
      <pre class="overflow-auto whitespace-pre-wrap px-3 pb-2 font-mono text-xs text-muted-foreground">{content}</pre>
    {/if}
  </div>
{/snippet}

<div class="rounded-md border border-border bg-card text-xs">
  <!-- Header row: tool name + status pill -->
  <div class="flex items-center gap-2 px-3 py-2">
    <span class="font-mono font-medium text-foreground">{name}</span>

    {#if status === "running"}
      <span class="flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 font-display text-muted-foreground">
        <Loader size={10} class="animate-spin" />
        running
      </span>
    {:else if status === "ok"}
      <span class="flex items-center gap-1 rounded-full bg-success px-2 py-0.5 font-display text-success-foreground">
        <CheckCircle size={10} />
        done
      </span>
    {:else}
      <span class="flex items-center gap-1 rounded-full bg-destructive px-2 py-0.5 font-display text-destructive-foreground">
        <XCircle size={10} />
        error
      </span>
    {/if}
  </div>

  {@render section("args", argsOpen, () => (argsOpen = !argsOpen), argsJson)}

  {#if !pending}
    {@render section("output", outputOpen, () => (outputOpen = !outputOpen), output ?? "")}
  {/if}
</div>
