<script lang="ts">
  import Block from "$lib/Block.svelte";
  import { sessionStore } from "$lib/stores/session.svelte";
  import { appStore, activeApp } from "$lib/stores/app.svelte";
  import Dashboard from "$lib/apps/finance/Dashboard.svelte";
  import ArtifactBrowser from "$lib/components/ArtifactBrowser.svelte";
  import { openExternal, copyLink } from "$lib/links.svelte";
  import { Button } from "$lib/components/ui/button";
  import ExternalLinkIcon from "@lucide/svelte/icons/external-link";
  import CopyIcon from "@lucide/svelte/icons/copy";
  import XIcon from "@lucide/svelte/icons/x";

  const promotedHost = $derived.by(() => {
    if (!sessionStore.promotedLink) return null;
    try {
      return new URL(sessionStore.promotedLink).host;
    } catch {
      return sessionStore.promotedLink;
    }
  });

  // Embed-failure state for the current link. Many sites refuse framing via
  // X-Frame-Options / CSP frame-ancestors and never fire `load`; we arm a
  // timeout and, if the iframe hasn't loaded in time, show a fallback instead of
  // an indefinitely blank panel.
  let embedFailed = $state(false);
  let loaded = $state(false);

  // Reset detection and arm the timeout whenever the promoted link changes.
  $effect(() => {
    const url = sessionStore.promotedLink;
    embedFailed = false;
    loaded = false;
    if (!url) return;
    const timer = setTimeout(() => {
      if (!loaded) embedFailed = true;
    }, 4000);
    return () => clearTimeout(timer);
  });

  function onIframeLoad() {
    loaded = true;
    // A site slower than the arm-timeout still recovers from the fallback once it loads.
    embedFailed = false;
  }

  // A hard failure (network/refused) fires `error`; surface the fallback at once.
  function onIframeError() {
    embedFailed = true;
  }

  function closeLink() {
    sessionStore.promotedLink = null;
  }
</script>

<div class="h-full bg-background">
  {#if sessionStore.promotedLink}
    <!-- C-12: the promoted link rendered as an embedded webview with a toolbar. -->
    <div class="flex h-full flex-col">
      <div class="flex items-center gap-2 border-b border-border px-3 py-2">
        <span class="min-w-0 flex-1 truncate font-mono text-sm text-foreground">{promotedHost}</span>
        <Button
          variant="outline"
          size="sm"
          onclick={() => openExternal(sessionStore.promotedLink!)}
          title="Open in browser"
        >
          <ExternalLinkIcon class="size-4" />
          Open in browser
        </Button>
        <Button
          variant="outline"
          size="sm"
          onclick={() => copyLink(sessionStore.promotedLink!)}
          title="Copy link"
        >
          <CopyIcon class="size-4" />
          Copy link
        </Button>
        <Button variant="ghost" size="icon" class="size-7" onclick={closeLink} title="Close">
          <XIcon class="size-4" />
        </Button>
      </div>
      <div class="relative min-h-0 flex-1">
        {#if embedFailed}
          <div class="flex h-full flex-col items-center justify-center gap-3 p-6 text-center">
            <p class="text-sm font-medium text-foreground">This site can't be embedded here</p>
            <p class="max-w-xs text-xs text-muted-foreground">
              The page refused to load in an embedded view. Open it in your browser instead.
            </p>
            <Button size="sm" onclick={() => openExternal(sessionStore.promotedLink!)}>
              <ExternalLinkIcon class="size-4" />
              Open in browser
            </Button>
          </div>
        {:else}
          <iframe
            src={sessionStore.promotedLink}
            title={promotedHost ?? "Embedded page"}
            sandbox="allow-scripts allow-same-origin allow-forms allow-popups"
            referrerpolicy="no-referrer"
            onload={onIframeLoad}
            onerror={onIframeError}
            class="h-full w-full border-0"
          ></iframe>
        {/if}
      </div>
    </div>
  {:else if sessionStore.panelMode === "browser"}
    <!-- A-4: artifact browser hosted in the panel. -->
    <ArtifactBrowser onClose={() => (sessionStore.panelMode = null)} />
  {:else if sessionStore.canvas}
    <div class="h-full overflow-auto p-4">
      <!-- Agent-chosen canvas view: not user-pinnable (no Pin overlay/wrapper),
           matching the artifact-browser preview. -->
      <Block block={sessionStore.canvas} canPin={false} />
    </div>
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
