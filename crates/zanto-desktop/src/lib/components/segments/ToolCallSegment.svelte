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

  // The whole card is collapsed by default — the header (name + status pill)
  // shows; click it to reveal the args/output sections. Within an open card,
  // args/output have their own toggles (args open, output closed).
  let cardOpen = $state(false);
  let argsOpen = $state(true);
  let outputOpen = $state(false);
</script>

{#snippet section(label: string, open: boolean, toggle: () => void, content: string)}
  <div>
    <button
      type="button"
      aria-expanded={open}
      onclick={toggle}
      class="flex w-full items-center gap-1 rounded-sm px-2 py-1 text-left font-mono text-xs text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <ChevronRight size={12} class={cn("transition-transform", open && "rotate-90")} />
      {label}
    </button>
    {#if open}
      <pre class="overflow-auto whitespace-pre-wrap px-2 pb-1 font-mono text-xs text-muted-foreground">{content}</pre>
    {/if}
  </div>
{/snippet}

<div class="text-xs">
  <!-- Header row: collapse toggle — tool name + status pill. Borderless row, not
       a card; hierarchy comes from indentation of the expanded sections below. -->
  <button
    type="button"
    aria-expanded={cardOpen}
    onclick={() => (cardOpen = !cardOpen)}
    class="flex w-full items-center gap-2 rounded-md px-2 py-1 text-left transition-colors hover:bg-muted/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
  >
    <ChevronRight size={12} class={cn("shrink-0 text-muted-foreground transition-transform", cardOpen && "rotate-90")} />
    <span class="font-mono font-medium text-foreground">{name}</span>

    {#if status === "running"}
      <span class="flex items-center gap-1 rounded-full bg-warning-soft px-2 py-0.5 font-display text-warning-soft-foreground">
        <Loader size={10} class="animate-spin" />
        running
      </span>
    {:else if status === "ok"}
      <span class="flex items-center gap-1 rounded-full bg-success-soft px-2 py-0.5 font-display text-success-soft-foreground">
        <CheckCircle size={10} />
        done
      </span>
    {:else}
      <span class="flex items-center gap-1 rounded-full bg-destructive-soft px-2 py-0.5 font-display text-destructive-soft-foreground">
        <XCircle size={10} />
        error
      </span>
    {/if}
  </button>

  {#if cardOpen}
    <!-- args/output indented under the tool row with a faint left guide. -->
    <div class="ml-3 border-l border-border/50 pl-2">
      {@render section("args", argsOpen, () => (argsOpen = !argsOpen), argsJson)}

      {#if !pending}
        {@render section("output", outputOpen, () => (outputOpen = !outputOpen), output ?? "")}
      {/if}
    </div>
  {/if}
</div>
