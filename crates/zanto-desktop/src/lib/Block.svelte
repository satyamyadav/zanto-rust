<script lang="ts">
  import { marked } from "marked";
  import DOMPurify from "dompurify";
  import { toast } from "svelte-sonner";
  import PinIcon from "@lucide/svelte/icons/pin";
  import type { ChatBlock } from "./ipc";
  import { ipc } from "./ipc";
  import { componentRegistry } from "./registry";
  import { validateArtifact, viewArtifacts } from "./stores/artifacts.svelte";
  import { interceptLinks } from "./links.svelte";
  import Json from "./blocks/Json.svelte";

  // `canPin` lets hosts (e.g. the artifact browser preview, which shows an
  // already-pinned view) suppress the A-5 Pin button. Defaults on.
  let { block, canPin = true }: { block: ChatBlock; canPin?: boolean } = $props();

  const html = $derived(
    block.kind === "markdown" ? DOMPurify.sanitize(marked.parse(block.text) as string) : ""
  );
  const Comp = $derived(block.kind === "component" ? componentRegistry[block.component_id] : undefined);
  const valid = $derived(
    block.kind === "component" ? validateArtifact(block.component_id, block.data) : true
  );
  const unknown = $derived(block.kind === "component" && !Comp);

  // A-5: show the user Pin button only on rendered VIEW-class component artifacts
  // (never file/markdown docs), and only when the host allows it.
  const pinnable = $derived(
    canPin &&
      block.kind === "component" &&
      Boolean(Comp) &&
      valid &&
      viewArtifacts.has(block.component_id)
  );

  let pinning = $state(false);
  async function pin() {
    if (block.kind !== "component" || pinning) return;
    pinning = true;
    try {
      await ipc.pinArtifact(block.component_id, block.data);
      toast.success("Pinned");
    } catch (e) {
      toast.error("Could not pin", { description: `${e}` });
    } finally {
      pinning = false;
    }
  }
</script>

{#if block.kind === "component"}
  {#if Comp && valid}
    {#if pinnable}
      <div class="group relative">
        <button
          type="button"
          onclick={pin}
          disabled={pinning}
          title="Pin to Artifacts"
          aria-label="Pin to Artifacts"
          class="absolute right-1 top-1 z-10 rounded-md border border-border bg-background/80 p-1 text-muted-foreground opacity-0 backdrop-blur transition-opacity hover:text-foreground focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring group-hover:opacity-100 disabled:opacity-50"
        >
          <PinIcon class="size-4" />
        </button>
        <Comp data={block.data} />
      </div>
    {:else}
      <Comp data={block.data} />
    {/if}
  {:else}
    {#if unknown}
      <div class="mb-1 font-mono text-xs text-muted-foreground">
        Unknown artifact: {block.component_id}
      </div>
    {/if}
    <Json data={block.data} />
  {/if}
{:else}
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  <div use:interceptLinks class="prose-zanto">{@html html}</div>
{/if}
