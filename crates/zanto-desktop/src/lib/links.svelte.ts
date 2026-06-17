// Link handling: intercept clicks on links inside rendered ({@html}) content so
// the Tauri webview never navigates the app, and route them to a controlled
// preview. http(s) links open the LinkPreview; other schemes are refused.
import { openUrl } from "@tauri-apps/plugin-opener";

// The single link currently shown in the preview popup (null = closed). A module
// store rather than per-component state so the intercept action (which runs
// outside any component) and the single mounted LinkPreview share one source.
export const linkPreview = $state<{ url: string | null }>({ url: null });

/** Open the dismissable preview for an http(s) url. */
export function showLinkPreview(url: string) {
  linkPreview.url = url;
}

/** Close the preview popup. */
export function dismissLinkPreview() {
  linkPreview.url = null;
}

/** True for http/https urls — the only schemes we open. */
function isHttp(url: string): boolean {
  try {
    const scheme = new URL(url, window.location.href).protocol;
    return scheme === "http:" || scheme === "https:";
  } catch {
    return false;
  }
}

/** Open a url in the system browser via the bundled opener plugin. */
export async function openExternal(url: string): Promise<void> {
  if (!isHttp(url)) return;
  await openUrl(url);
}

/**
 * Svelte action: applied to a container of sanitized {@html} content, it
 * delegates click events on descendant `<a href>` elements — preventing the
 * default navigation and showing the preview for http(s) links. Non-http(s)
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
    if (isHttp(url)) showLinkPreview(url);
  }

  node.addEventListener("click", onClick);
  return {
    destroy() {
      node.removeEventListener("click", onClick);
    },
  };
}
