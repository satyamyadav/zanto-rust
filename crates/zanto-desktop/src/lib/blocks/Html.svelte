<script lang="ts">
  // Renders an agent-produced HTML page inside a LOCKED-DOWN sandboxed iframe.
  //
  // Security model (the whole point of this component — do not weaken):
  //  • sandbox="allow-scripts" and NOTHING else. Critically NOT
  //    allow-same-origin: with both, a script inside could remove its own
  //    sandbox. Omitting same-origin makes the frame a null origin — it cannot
  //    read window.parent, the app DOM, localStorage, cookies, or the Tauri
  //    bridge (window.__TAURI__ is undefined inside it).
  //  • An injected CSP <meta> at the very top of <head> blocks all network
  //    egress: default-src 'none' (no fetch/XHR/websocket/remote anything),
  //    only inline <script>/<style>, images/fonts as data: URIs. A second CSP
  //    the agent might inject can only intensify, never relax, this one.
  //  • The content goes into the iframe `srcdoc` ATTRIBUTE (a string) — never
  //    into the host DOM via {@html}. So nothing the agent sends touches the app.
  //
  // Net: interactive HTML/JS runs, but has nothing to touch and nowhere to send.

  type HtmlData = {
    title?: string;
    content: string;
  };

  let { data }: { data: HtmlData } = $props();

  // The CSP we force at the top of the document. `default-src 'none'` denies
  // everything not explicitly re-allowed; we re-allow only inline script/style
  // and data: images/fonts. No connect-src ⇒ no network. No remote src of any
  // kind. `frame-src 'none'` stops nested frames.
  const CSP =
    "default-src 'none'; " +
    "script-src 'unsafe-inline'; " +
    "style-src 'unsafe-inline'; " +
    "img-src data:; " +
    "font-src data:; " +
    "frame-src 'none'; " +
    "base-uri 'none'; " +
    "form-action 'none'";

  const META = `<meta http-equiv="Content-Security-Policy" content="${CSP}">`;

  // Build the final srcdoc: our CSP meta must come FIRST in <head> so it binds.
  // Tolerate three shapes of agent content: a full document with <head>, a full
  // document with <html> but no <head>, or a bare fragment.
  const srcdoc = $derived.by(() => {
    const raw = data.content ?? "";
    if (/<head[\s>]/i.test(raw)) {
      // Inject our meta immediately after the opening <head> tag.
      return raw.replace(/<head([^>]*)>/i, `<head$1>${META}`);
    }
    if (/<html[\s>]/i.test(raw)) {
      // Has <html> but no <head>: add a <head> carrying the CSP right after it.
      return raw.replace(/<html([^>]*)>/i, `<html$1><head>${META}</head>`);
    }
    // Bare fragment: wrap in a minimal document with the CSP head.
    return `<!doctype html><html><head>${META}<meta charset="utf-8"></head><body>${raw}</body></html>`;
  });
</script>

<div class="flex h-full min-h-0 flex-col">
  {#if data.title}
    <div class="mb-2 shrink-0 text-sm font-medium text-foreground">{data.title}</div>
  {/if}
  <!-- sandbox carries ONLY allow-scripts. Never add allow-same-origin. -->
  <iframe
    title={data.title ?? "HTML artifact"}
    sandbox="allow-scripts"
    referrerpolicy="no-referrer"
    {srcdoc}
    class="h-full min-h-[16rem] w-full flex-1 rounded-md border border-border bg-white"
  ></iframe>
</div>
