<script lang="ts">
  // Dismissable preview for an intercepted link. Mounted once (in Canvas) and
  // driven by the shared `linkPreview` store; the webview never navigates, so
  // every link goes through this confirmation before opening externally.
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { toast } from "svelte-sonner";
  import { sessionStore } from "$lib/stores/session.svelte";
  import { linkPreview, dismissLinkPreview, openExternal } from "$lib/links.svelte";
  import ExternalLinkIcon from "@lucide/svelte/icons/external-link";
  import CopyIcon from "@lucide/svelte/icons/copy";
  import PanelRightIcon from "@lucide/svelte/icons/panel-right";

  const open = $derived(linkPreview.url !== null);

  // Split the active url into emphasized host + muted full url for display.
  const parsed = $derived.by(() => {
    if (!linkPreview.url) return null;
    try {
      return new URL(linkPreview.url);
    } catch {
      return null;
    }
  });

  function onOpenChange(v: boolean) {
    if (!v) dismissLinkPreview();
  }

  async function openInBrowser() {
    const url = linkPreview.url;
    if (!url) return;
    try {
      await openExternal(url);
      dismissLinkPreview();
    } catch (e) {
      toast.error("Could not open the link", { description: `${e}` });
    }
  }

  async function copyLink() {
    const url = linkPreview.url;
    if (!url) return;
    try {
      await navigator.clipboard.writeText(url);
      toast.success("Link copied");
    } catch (e) {
      toast.error("Could not copy the link", { description: `${e}` });
    }
  }

  function viewInPanel() {
    if (!linkPreview.url) return;
    sessionStore.promotedLink = linkPreview.url;
    sessionStore.canvas = null;
    dismissLinkPreview();
  }
</script>

<Dialog.Root {open} {onOpenChange}>
  <Dialog.Content class="max-w-sm">
    <Dialog.Header>
      <Dialog.Title class="font-display">Open link</Dialog.Title>
    </Dialog.Header>

    {#if parsed}
      <div class="space-y-1">
        <div class="font-mono text-sm font-semibold text-foreground">{parsed.host}</div>
        <div class="break-all font-mono text-xs text-muted-foreground">{linkPreview.url}</div>
      </div>
    {/if}

    <Dialog.Footer class="flex flex-wrap gap-2 sm:justify-start">
      <Button size="sm" onclick={openInBrowser}>
        <ExternalLinkIcon />
        Open in browser
      </Button>
      <Button size="sm" variant="outline" onclick={copyLink}>
        <CopyIcon />
        Copy link
      </Button>
      <Button size="sm" variant="outline" onclick={viewInPanel}>
        <PanelRightIcon />
        View in panel
      </Button>
      <Button size="sm" variant="ghost" onclick={dismissLinkPreview}>Dismiss</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
