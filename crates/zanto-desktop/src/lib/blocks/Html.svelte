<script lang="ts">
  // Renders an agent-produced HTML page inside a LOCKED-DOWN sandboxed iframe.
  //
  // Security model (the whole point of this component — do not weaken):
  //  • sandbox="allow-scripts" and NOTHING else. Critically NOT
  //    allow-same-origin: with both, a script inside could remove its own
  //    sandbox. Omitting same-origin makes the frame a null origin — it cannot
  //    read window.parent, the app DOM, localStorage, cookies, or the Tauri
  //    bridge (window.__TAURI__ is undefined inside it).
  //  • The agent content is placed in the BODY of a fixed document shell we fully
  //    control, whose <head> carries our CSP <meta> FIRST. We never parse, rewrite,
  //    or trust the agent's markup to find an injection point — so no crafted
  //    input (a commented or attribute-embedded <head>, a self-supplied CSP, etc.)
  //    can displace or neutralize our policy. A <meta http-equiv=CSP> only binds
  //    from <head>; any CSP the agent puts in its (body-position) content is
  //    ignored by the parser, and the first policy wins regardless.
  //  • That CSP blocks all network egress: default-src 'none' (no fetch/XHR/
  //    websocket/remote anything), only inline <script>/<style>, data: img/fonts.
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

  // Build the srcdoc by WRAPPING the agent content in a fixed document shell whose
  // <head> holds our CSP meta first. We do not inspect or rewrite the agent markup
  // at all — whatever it is (a bare fragment, or a full <html> document) becomes
  // body content. A full document nested in body position is non-conforming but
  // parses fine (browsers ignore the stray <html>/<head>/<body> and still run its
  // scripts/visible content); crucially, any <meta http-equiv=CSP> the agent
  // supplies sits in body position and is ignored, so OUR head-level CSP is the
  // only effective policy and is guaranteed present and first.
  const srcdoc = $derived(
    `<!doctype html><html><head>${META}<meta charset="utf-8"></head><body>${data.content ?? ""}</body></html>`,
  );
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
