// Link handling: intercept clicks on links inside rendered ({@html}) content so
// the Tauri webview never navigates the app, and route them into the right-hand
// canvas panel as an embedded webview. http(s) links open in the panel; other
// schemes are refused.
import { openUrl } from "@tauri-apps/plugin-opener";
import { toast } from "svelte-sonner";
import { sessionStore } from "$lib/stores/session.svelte";

/** True for http/https urls — the only schemes we open. */
function isHttp(url: string): boolean {
  try {
    const scheme = new URL(url, window.location.href).protocol;
    return scheme === "http:" || scheme === "https:";
  } catch {
    return false;
  }
}

/**
 * Promote an http(s) url into the canvas panel (C-12). Sets `promotedLink` and
 * clears the sibling panel views so precedence is unambiguous (link wins).
 */
export function openLinkInPanel(url: string) {
  if (!isHttp(url)) return;
  sessionStore.canvas = null;
  sessionStore.panelMode = null;
  sessionStore.promotedLink = url;
}

/** Open a url in the system browser via the bundled opener plugin. */
export async function openExternal(url: string): Promise<void> {
  if (!isHttp(url)) return;
  await openUrl(url);
}

/** Copy a url to the clipboard, toasting the outcome. */
export async function copyLink(url: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(url);
    toast.success("Link copied");
  } catch (e) {
    toast.error("Could not copy the link", { description: `${e}` });
  }
}

/**
 * Svelte action: applied to a container of sanitized {@html} content, it
 * delegates click events on descendant `<a href>` elements — preventing the
 * default navigation and opening http(s) links in the canvas panel. Non-http(s)
 * schemes (mailto:, javascript:, etc.) are refused outright. Delegation means
 * one listener handles links injected after mount.
 */
export function interceptLinks(node: HTMLElement) {
  function onClick(e: MouseEvent) {
    const anchor = (e.target as Element | null)?.closest("a[href]");
    if (!anchor || !node.contains(anchor)) return;
    e.preventDefault();
    const href = anchor.getAttribute("href");
    if (!href) return;
    let url: string;
    try {
      url = new URL(href, window.location.href).href;
    } catch {
      return;
    }
    if (isHttp(url)) openLinkInPanel(url);
  }

  node.addEventListener("click", onClick);
  return {
    destroy() {
      node.removeEventListener("click", onClick);
    },
  };
}
