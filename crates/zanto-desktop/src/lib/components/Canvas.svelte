<script lang="ts">
  import Block from "$lib/Block.svelte";
  import { sessionStore } from "$lib/stores/session.svelte";
  import { appStore, activeApp } from "$lib/stores/app.svelte";
  import Dashboard from "$lib/apps/finance/Dashboard.svelte";
  import LinkPreview from "$lib/components/LinkPreview.svelte";
  import { openExternal } from "$lib/links.svelte";
  import { Button } from "$lib/components/ui/button";
  import ExternalLinkIcon from "@lucide/svelte/icons/external-link";
  import XIcon from "@lucide/svelte/icons/x";

  const promotedHost = $derived.by(() => {
    if (!sessionStore.promotedLink) return null;
    try {
      return new URL(sessionStore.promotedLink).host;
    } catch {
      return sessionStore.promotedLink;
    }
  });
</script>

<!-- Mounted once; the dialog itself portals, so its position here is irrelevant. -->
<LinkPreview />

<div class="h-full overflow-auto bg-background p-4">
  {#if sessionStore.promotedLink}
    <div class="rounded-lg border border-border bg-card p-4">
      <div class="mb-1 flex items-center justify-between gap-2">
        <span class="font-display text-sm font-medium text-foreground">Link</span>
        <Button
          variant="ghost"
          size="icon"
          class="size-6"
          onclick={() => (sessionStore.promotedLink = null)}
          title="Close"
        >
          <XIcon class="size-4" />
        </Button>
      </div>
      <div class="font-mono text-sm font-semibold text-foreground">{promotedHost}</div>
      <div class="mb-3 break-all font-mono text-xs text-muted-foreground">
        {sessionStore.promotedLink}
      </div>
      <Button size="sm" onclick={() => openExternal(sessionStore.promotedLink!)}>
        <ExternalLinkIcon />
        Open in browser
      </Button>
    </div>
  {:else if sessionStore.canvas}
    <Block block={sessionStore.canvas} />
  {:else if appStore.activeId === "finance"}
    <Dashboard />
  {:else}
    <div class="flex h-full items-center justify-center p-6">
      <div class="max-w-xs text-center font-sans">
        <p class="text-sm font-medium text-foreground">
          {activeApp()?.name ?? "Nothing open yet"}
        </p>
        <p class="mt-1 text-sm text-muted-foreground">
          Views and artifacts open here — ask zanto to show data as a table or chart.
        </p>
      </div>
    </div>
  {/if}
</div>
