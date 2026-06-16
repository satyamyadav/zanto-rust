<script lang="ts">
  import { onMount } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { ipc, type InteractionRequest } from "$lib/ipc";

  // The single HITL surface above the composer: permission approvals and agent forms.
  let req = $state<InteractionRequest | null>(null);
  let stepIdx = $state(0);
  let answers = $state<Record<string, any>>({});

  onMount(() => {
    const un = ipc.onInteractionRequest((r) => {
      req = r;
      stepIdx = 0;
      answers = {};
    });
    return () => un.then((f) => f());
  });

  function approve(value: "once" | "session" | "forever" | "deny") {
    const r = req;
    req = null;
    if (r) ipc.respond(r.id, value);
  }

  function submitForm() {
    const r = req;
    req = null;
    if (r) ipc.respond(r.id, answers);
  }

  const steps = $derived(req?.steps ?? []);
  const isLast = $derived(stepIdx >= steps.length - 1);
</script>

{#if req}
  <div
    class="absolute bottom-full left-0 right-0 mb-2 mx-3 rounded-lg border border-border bg-popover text-popover-foreground shadow-lg p-3 z-20"
  >
    {#if req.kind === "approval"}
      <div class="text-sm mb-1">
        <span class="uppercase text-muted-foreground">{req.op}</span>
        <span class="font-mono">"{req.path}"</span>
      </div>
      <div class="text-xs text-muted-foreground font-mono break-all mb-2">{req.resolved}</div>
      <div class="flex gap-2">
        <Button size="sm" onclick={() => approve("once")}>Allow once</Button>
        <Button size="sm" variant="secondary" onclick={() => approve("session")}>Session</Button>
        <Button size="sm" variant="secondary" onclick={() => approve("forever")}>Forever</Button>
        <Button size="sm" variant="destructive" class="ml-auto" onclick={() => approve("deny")}>Deny</Button>
      </div>
    {:else}
      {#if req.title}<div class="text-sm font-medium mb-2">{req.title}</div>{/if}
      {#each steps[stepIdx]?.fields ?? [] as f}
        <div class="space-y-1 mb-2">
          <label class="text-xs text-muted-foreground" for={`hitl-${f.name}`}>{f.label}</label>
          {#if f.type === "select"}
            <select
              id={`hitl-${f.name}`}
              class="w-full border border-input rounded px-2 py-1 text-sm bg-background"
              bind:value={answers[f.name]}
            >
              {#each f.options ?? [] as o}<option value={o}>{o}</option>{/each}
            </select>
          {:else if f.type === "confirm"}
            <input id={`hitl-${f.name}`} type="checkbox" bind:checked={answers[f.name]} />
          {:else}
            <Input id={`hitl-${f.name}`} bind:value={answers[f.name]} />
          {/if}
        </div>
      {/each}
      <div class="flex gap-2 justify-end pt-1">
        {#if stepIdx > 0}
          <Button size="sm" variant="ghost" onclick={() => (stepIdx -= 1)}>Back</Button>
        {/if}
        {#if isLast}
          <Button size="sm" onclick={submitForm}>Submit</Button>
        {:else}
          <Button size="sm" onclick={() => (stepIdx += 1)}>Next</Button>
        {/if}
      </div>
    {/if}
  </div>
{/if}
