// Link handling: intercept clicks on links inside rendered ({@html}) content so
// the Tauri webview never navigates the app, and route them into the right-hand
// canvas panel as an embedded webview. http(s) links open in the panel; other
// schemes are refused.
import { ipc } from "$lib/ipc";
import { toast } from "svelte-sonner";
import { sessionStore } from "$lib/stores/session.svelte";
import { appStore } from "$lib/stores/app.svelte";

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
 * Regex matching the text content of a <code> element that is an absolute or
 * home-relative path. Matches:
 *   - Unix absolute: /foo/bar or /foo
 *   - Home-relative: ~/foo or ~/
 *   - Windows drive:  C:\foo or C:/foo (any ASCII letter drive)
 *
 * Does NOT match:
 *   - Relative paths (src/main.rs, ./foo, ../bar)
 *   - Bare prose slash-strings (not inside <code>)
 *
 * The regex is anchored so the entire code-element text must be a path —
 * no partial matches that could produce false positives.
 */
const ABS_PATH_RE = /^(\/[^\s`'"<>]|~\/[^\s`'"<>]|[A-Za-z]:[/\\][^\s`'"<>])/;

/**
 * Given an absolute path and the current project directory, return the shortest
 * label to display: if the path is under projectDir, show the relative form;
 * otherwise show the full path.
 *
 * Example:
 *   projectDir = "/home/user/project"
 *   path       = "/home/user/project/src/main.rs"
 *   → "src/main.rs"
 */
function displayPath(path: string, projectDir: string | null): string {
  if (projectDir) {
    const prefix = projectDir.endsWith("/") ? projectDir : projectDir + "/";
    if (path.startsWith(prefix)) {
      return path.slice(prefix.length);
    }
  }
  return path;
}

/**
 * Scan `<code>` elements that are direct inline children (not inside a `<pre>`)
 * within `node`. For each whose text content matches ABS_PATH_RE and has not
 * already been linkified, wrap it in an `<a>` with `data-file-path` pointing to
 * the absolute path and visible text set to the (possibly relative) display form.
 *
 * Anti-XSS contract:
 *   - We only read `code.textContent` (sanitized plain text from DOMPurify).
 *   - We validate against ABS_PATH_RE before touching the DOM.
 *   - We write `anchor.textContent` (never innerHTML) for the display label.
 *   - `data-file-path` carries the same already-sanitized text.
 *   - No model-supplied HTML is ever injected.
 */
function linkifyFilePaths(node: HTMLElement): void {
  const projectDir = appStore.config?.project_dir ?? null;
  // Only inline <code> — skip those inside <pre> (code blocks).
  const codes = node.querySelectorAll<HTMLElement>("code");
  for (const code of codes) {
    // Skip code blocks (inside <pre>).
    if (code.closest("pre")) continue;
    // Skip already-linkified elements.
    if (code.closest("a[data-file-path]")) continue;

    const text = code.textContent ?? "";
    if (!ABS_PATH_RE.test(text)) continue;

    // Safe: we own this DOM element (it came through DOMPurify).
    // Build the anchor, set only textContent (never innerHTML).
    const anchor = document.createElement("a");
    anchor.setAttribute("data-file-path", text);
    anchor.setAttribute("href", "#");
    anchor.className = code.className;
    anchor.textContent = displayPath(text, projectDir);
    code.replaceWith(anchor);
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
  await ipc.openExternal(url);
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
 * Svelte action: applied to a container of sanitized {@html} content, it:
 *   1. Linkifies backticked absolute paths (via linkifyFilePaths) on mount and
 *      after each DOM mutation (to catch streamed content).
 *   2. Delegates click events on descendant `<a href>` elements — preventing the
 *      default navigation and routing them appropriately:
 *        - `[data-file-path]` anchors → ipc.openPath (OS default app)
 *        - http(s) links → openLinkInPanel (canvas panel, C-12)
 *        - other schemes → refused outright
 */
export function interceptLinks(node: HTMLElement) {
  // Initial linkification (for non-streaming, already-rendered content).
  linkifyFilePaths(node);

  // MutationObserver: re-run linkification when streamed chunks add new DOM nodes.
  const obs = new MutationObserver(() => linkifyFilePaths(node));
  obs.observe(node, { childList: true, subtree: true });

  function onClick(e: MouseEvent) {
    const anchor = (e.target as Element | null)?.closest("a[href]");
    if (!anchor || !node.contains(anchor)) return;
    e.preventDefault();

    // File-path link: open with OS default app.
    const filePath = anchor.getAttribute("data-file-path");
    if (filePath) {
      ipc.openPath(filePath).catch((err) => {
        toast.error("Could not open file", { description: `${err}` });
      });
      return;
    }

    // HTTP(S) link: open in the canvas panel.
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
      obs.disconnect();
      node.removeEventListener("click", onClick);
    },
  };
}
