<script lang="ts">
  import Block from "$lib/Block.svelte";
  import { sessionStore } from "$lib/stores/session.svelte";
  import { appStore, activeApp } from "$lib/stores/app.svelte";
  import Dashboard from "$lib/apps/finance/Dashboard.svelte";
  import ArtifactHub from "$lib/components/ArtifactHub.svelte";
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

  function closeLink() {
    sessionStore.promotedLink = null;
  }

  // When a rendered canvas view AND the Artifacts browser both exist, the panel
  // tabs between them. Opening the browser focuses its tab; closing it (or having
  // no browser) falls back to the rendered View.
  let panelTab = $state<"view" | "artifacts">("view");
  $effect(() => {
    panelTab = sessionStore.panelMode === "browser" ? "artifacts" : "view";
  });
</script>

<div class="h-full bg-background p-4">
  {#if sessionStore.promotedLink}
    <!-- C-12: external links can't be reliably embedded in the WebKitGTK webview
         (X-Frame-Options/CSP → blank page), so the panel is a clean open-card
         rather than an iframe. -->
    <div class="flex h-full flex-col">
      <div class="flex items-center gap-2 border-b border-border px-3 py-2">
        <span class="min-w-0 flex-1 truncate font-mono text-sm text-foreground">{promotedHost}</span>
        <Button variant="ghost" size="icon" class="size-7" onclick={closeLink} title="Close">
          <XIcon class="size-4" />
        </Button>
      </div>
      <div class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 p-6 text-center">
        <div class="rounded-full bg-accent p-3">
          <ExternalLinkIcon class="size-6 text-accent-foreground" />
        </div>
        <p class="max-w-xs break-all font-mono text-sm text-foreground">{sessionStore.promotedLink}</p>
        <p class="max-w-xs text-xs text-muted-foreground">
          This page opens in your browser — pages can't be displayed inside the app.
        </p>
        <div class="flex flex-wrap justify-center gap-2">
          <Button size="sm" onclick={() => openExternal(sessionStore.promotedLink!)}>
            <ExternalLinkIcon class="size-4" />
            Open in browser
          </Button>
          <Button variant="outline" size="sm" onclick={() => copyLink(sessionStore.promotedLink!)}>
            <CopyIcon class="size-4" />
            Copy link
          </Button>
        </div>
      </div>
    </div>
  {:else if sessionStore.canvas && sessionStore.panelMode === "browser"}
    <!-- Both a rendered view and the Artifacts browser exist: tab between them so
         opening Artifacts never destroys the rendered canvas. -->
    <div class="flex h-full flex-col">
      <div class="flex shrink-0 items-center gap-1 border-b border-border px-2 py-1.5" role="tablist">
        <button
          type="button"
          role="tab"
          aria-selected={panelTab === "view"}
          onclick={() => (panelTab = "view")}
          class="rounded-md px-2.5 py-1 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {panelTab === 'view'
            ? 'bg-accent text-accent-foreground'
            : 'text-muted-foreground hover:text-foreground'}"
        >View</button>
        <button
          type="button"
          role="tab"
          aria-selected={panelTab === "artifacts"}
          onclick={() => (panelTab = "artifacts")}
          class="rounded-md px-2.5 py-1 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {panelTab === 'artifacts'
            ? 'bg-accent text-accent-foreground'
            : 'text-muted-foreground hover:text-foreground'}"
        >Artifacts</button>
      </div>
      <div class="min-h-0 flex-1 overflow-auto">
        {#if panelTab === "view"}
          <div class="h-full overflow-auto p-4"><Block block={sessionStore.canvas} canPin={false} /></div>
        {:else}
          <ArtifactHub onClose={() => (sessionStore.panelMode = null)} />
        {/if}
      </div>
    </div>
  {:else if sessionStore.panelMode === "browser"}
    <!-- A-4: artifact hub hosted in the panel. -->
    <ArtifactHub onClose={() => (sessionStore.panelMode = null)} />
  {:else if sessionStore.canvas}
    <div class="h-full overflow-auto p-4">
      <!-- Agent-chosen canvas view: not user-pinnable (no Pin overlay/wrapper),
           matching the artifact-browser preview. -->
      <Block block={sessionStore.canvas} canPin={false} />
    </div>
  {:else if appStore.activeId === "finance"}
    <div data-testid="canvas-scroll" class="h-full min-h-0 overflow-y-auto">
      <Dashboard />
    </div>
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
