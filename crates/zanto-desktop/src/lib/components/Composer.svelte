<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Textarea } from "$lib/components/ui/textarea";
  import { toast } from "svelte-sonner";
  import SendIcon from "@lucide/svelte/icons/send";
  import SquareIcon from "@lucide/svelte/icons/square";
  import PaperclipIcon from "@lucide/svelte/icons/paperclip";
  import XIcon from "@lucide/svelte/icons/x";
  import FileIcon from "@lucide/svelte/icons/file";
  import ImageIcon from "@lucide/svelte/icons/image";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import TerminalIcon from "@lucide/svelte/icons/terminal";
  import LoaderIcon from "@lucide/svelte/icons/loader";
  import BookOpenIcon from "@lucide/svelte/icons/book-open";
  import PencilIcon from "@lucide/svelte/icons/pencil";
  import { onMount } from "svelte";
  import { skillsStore, openSkillsEditor } from "$lib/stores/skills.svelte";
  import { sessionStore, sessionUsage, send, newSession, interrupt } from "$lib/stores/session.svelte";
  import { appStore } from "$lib/stores/app.svelte";
  import { openSettings } from "$lib/stores/settings.svelte";
  import { ipc, type FileEntry, type SkillDto } from "$lib/ipc";
  import FileListItem from "$lib/components/FileListItem.svelte";

  // Active-context summary: enabled context sources + the project's base name.
  // Opens the Workspace dialog so it's obvious what's feeding the agent.
  const enabledCount = $derived(
    (appStore.config?.context_sources ?? []).filter((s) => s.enabled).length,
  );
  const projectName = $derived(
    appStore.config?.project_dir?.split(/[/\\]/).filter(Boolean).pop() ?? null,
  );
  const contextLabel = $derived(
    [
      enabledCount > 0 ? `${enabledCount} source${enabledCount === 1 ? "" : "s"}` : null,
      projectName,
    ]
      .filter(Boolean)
      .join(" · ") || "No active context",
  );

  // Session token gauge: cumulative tokens used / the active model's context
  // window. Shown only once a turn has reported usage (window + total known).
  // `~` marks an estimate (any contributing turn estimated). The bar fills by the
  // last-known window occupancy; the tooltip carries the cumulative-vs-window note.
  function fmtTokens(n: number): string {
    if (n >= 1000) return `${(n / 1000).toFixed(n >= 10000 ? 0 : 1)}k`;
    return `${n}`;
  }
  const usage = $derived(sessionUsage());
  const showGauge = $derived(usage.total > 0 && sessionStore.windowTokens > 0);
  const gaugePct = $derived(
    sessionStore.windowTokens > 0
      ? Math.min(100, Math.round((usage.total / sessionStore.windowTokens) * 100))
      : 0,
  );
  const gaugeLabel = $derived(
    `${usage.estimated ? "~" : ""}${fmtTokens(usage.total)} / ${fmtTokens(sessionStore.windowTokens)}`,
  );
  const gaugeTitle = $derived(
    `${usage.total.toLocaleString()} tokens used across ${usage.turns} turn${usage.turns === 1 ? "" : "s"}` +
      ` · ${sessionStore.windowTokens.toLocaleString()}-token context window (${gaugePct}%)` +
      (usage.estimated ? " · includes estimated counts" : ""),
  );

  // Large pastes become collapsed chips instead of flooding the textarea; the
  // full text is still spliced into the final message on send.
  const CHAR_THRESHOLD = 2000;
  const LINE_THRESHOLD = 20;

  type Paste = { id: number; text: string; lines: number };

  // Attached files (button or drag-drop). Documents are lazy `@path` references
  // (the agent reads them with `read_document`); images ride the user message as
  // vision input when the model supports it (see session.send / send_message).
  type Attachment = { id: number; path: string; name: string; isImage: boolean };

  // Image extensions sent as vision input rather than `@path` document refs.
  const IMAGE_EXTS = ["png", "jpg", "jpeg", "webp", "gif", "bmp"];
  function isImagePath(path: string): boolean {
    const ext = path.split(".").pop()?.toLowerCase() ?? "";
    return IMAGE_EXTS.includes(ext);
  }

  let input = $state("");
  let pastes = $state<Paste[]>([]);
  let attachments = $state<Attachment[]>([]);
  let dragOver = $state(false);
  let nextId = 0;
  let textarea = $state<HTMLTextAreaElement | null>(null);

  function basename(path: string): string {
    return path.split(/[/\\]/).filter(Boolean).pop() ?? path;
  }

  function dirname(path: string): string {
    const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return i > 0 ? path.slice(0, i) : path;
  }

  // Add picked/dropped paths as chips and auto-grant read on each (so
  // `read_document` needs no separate approval). Skips already-attached paths.
  async function addAttachments(paths: string[]) {
    for (const path of paths) {
      if (!path || attachments.some((a) => a.path === path)) continue;
      attachments = [
        ...attachments,
        { id: nextId++, path, name: basename(path), isImage: isImagePath(path) },
      ];
      try {
        await ipc.addAllowedPath(dirname(path));
      } catch (e) {
        toast.error(`${e}`);
      }
    }
  }

  function removeAttachment(id: number) {
    attachments = attachments.filter((a) => a.id !== id);
  }

  async function pickFiles() {
    try {
      const paths = await ipc.pickFiles();
      if (paths.length > 0) await addAttachments(paths);
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  onMount(() => {
    const unlisten = ipc.onFileDrop({
      onEnter: () => (dragOver = true),
      onLeave: () => (dragOver = false),
      onDrop: (paths) => void addAttachments(paths),
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  });

  function lineCount(text: string): number {
    return text.split("\n").length;
  }

  function isLarge(text: string): boolean {
    return text.length > CHAR_THRESHOLD || lineCount(text) > LINE_THRESHOLD;
  }

  function onpaste(e: ClipboardEvent) {
    const text = e.clipboardData?.getData("text/plain") ?? "";
    if (!isLarge(text)) return; // small pastes behave normally
    e.preventDefault();
    pastes = [...pastes, { id: nextId++, text, lines: lineCount(text) }];
  }

  function removePaste(id: number) {
    pastes = pastes.filter((p) => p.id !== id);
  }

  function composeMessage(): string {
    const typed = input.trim();
    const pasted = pastes.map((p) => p.text).join("\n\n");
    // Document attachments become `@<path>` tokens (same convention as the @-tag
    // picker) so the agent reads them via `read_document`. Image attachments are
    // sent separately as vision input (see submit), not as `@path` tokens.
    const attached = attachments
      .filter((a) => !a.isImage)
      .map((a) => `@${a.path}`)
      .join(" ");
    return [typed, pasted, attached].filter((s) => s.length > 0).join("\n\n");
  }

  async function submit() {
    // While busy, Enter queues the message (send() handles the FIFO queue); the
    // Stop button — not submit — interrupts the running turn.
    const text = composeMessage();
    // Image attachments carry no `@path` text, so allow a send with images even
    // when the composed text is empty.
    const imagePaths = attachments.filter((a) => a.isImage).map((a) => a.path);
    if (!text && imagePaths.length === 0) return;
    // Snapshot attachment metadata for the user bubble before clearing the composer.
    const attachmentMeta = attachments.map((a) => ({ path: a.path, name: a.name, isImage: a.isImage }));
    input = "";
    pastes = [];
    attachments = [];
    closeMenu();
    try {
      await send(text, imagePaths, attachmentMeta);
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  // ── Composer menus (@ file-picker / slash commands / skill picker) ────────
  // A single overlay drives all menus; `menu` selects which is active.
  type Menu = "none" | "file" | "slash" | "skill";

  // Slash-command registry. `/clear` is always selectable — gating it on
  // "is there something to clear" made typing `/clear` strip its own fragment,
  // emptying the check and hiding the command (a deadlock). `clearInput` is a
  // harmless no-op when the composer is already empty.
  type SlashCommand = { name: string; hint: string; run: () => void };
  const SLASH_COMMANDS = $derived<SlashCommand[]>([
    { name: "new", hint: "Start a new session", run: () => newSession() },
    { name: "clear", hint: "Clear the composer", run: clearInput },
    { name: "skill", hint: "Select an active skill", run: openSkillMenu },
  ]);

  let menu = $state<Menu>("none");
  let active = $state(0); // highlighted item index
  // @-menu state: the directory listing, what's been typed after `@`, the
  // `@` position in `input`, and the current directory (null = roots).
  let entries = $state<FileEntry[]>([]);
  let dirStack = $state<string[]>([]); // breadcrumb of descended paths
  let tagStart = -1; // index of the `@` that opened the file menu
  let query = $state(""); // text typed after `@` (or `/`)
  let loadingDir = $state(false); // a directory listing fetch is in flight
  let loadSeq = 0; // monotonically-increasing counter; guards against stale loadDir responses
  // Skill-menu state
  let skills = $state<SkillDto[]>([]);
  let activeSkillName = $state<string | null>(null); // currently selected skill name
  let skillQuery = $state(""); // text typed after opening the skill picker (separate from slash/file query)

  function clearInput() {
    input = "";
    pastes = [];
    attachments = [];
  }

  function closeMenu() {
    menu = "none";
    active = 0;
    query = "";
    tagStart = -1;
  }

  // When query contains `/`, the trailing segment after the last `/` is the filter;
  // the leading segment(s) are directory names to descend into.
  // `pathFilter` is what we actually filter entries by (the trailing segment or the full query).
  const pathFilter = $derived(
    menu === "file" && query.includes("/") ? query.split("/").pop()! : query,
  );
  const filteredEntries = $derived(
    pathFilter
      ? entries.filter((e) => e.name.toLowerCase().includes(pathFilter.toLowerCase()))
      : entries,
  );
  const filteredCommands = $derived(
    query
      ? SLASH_COMMANDS.filter((c) => c.name.toLowerCase().includes(query.toLowerCase()))
      : SLASH_COMMANDS,
  );
  // The picker selects a skill by NAME, and the runtime resolves a name with
  // project shadowing global (get_skill) — so two same-named skills are one
  // choice here, not two. listSkills returns both scope copies (the editor needs
  // them); collapse by name keeping the first occurrence. The IPC lists project
  // before global, so the project copy wins — matching runtime resolution. This
  // also keeps the {#each (s.name)} keys unique (a duplicate key would crash).
  const dedupedSkills = $derived.by(() => {
    const byName = new Map<string, SkillDto>();
    for (const s of skills) if (!byName.has(s.name)) byName.set(s.name, s);
    return [...byName.values()];
  });
  const filteredSkills = $derived(
    skillQuery
      ? dedupedSkills.filter((s) => s.name.toLowerCase().includes(skillQuery.toLowerCase()))
      : dedupedSkills,
  );
  const itemCount = $derived(
    menu === "file"
      ? filteredEntries.length
      : menu === "skill"
        ? filteredSkills.length
        : filteredCommands.length,
  );

  async function loadDir(path?: string) {
    const seq = ++loadSeq;
    loadingDir = true;
    try {
      const result = await ipc.browseDir(path);
      // Only apply if no newer loadDir call has started since this one was dispatched.
      if (seq === loadSeq) entries = result;
    } catch (e) {
      if (seq === loadSeq) {
        toast.error(`${e}`);
        closeMenu();
      }
    } finally {
      if (seq === loadSeq) loadingDir = false;
    }
  }

  // Pop the top of dirStack and reload the parent directory (or roots if empty).
  async function ascendDir() {
    const next = dirStack.slice(0, -1);
    dirStack = next;
    query = "";
    active = 0;
    // Clear any typed fragment after `@` so the token stays clean.
    const caret = textarea ? textarea.selectionStart : input.length;
    if (tagStart >= 0 && caret > tagStart + 1) {
      input = input.slice(0, tagStart + 1) + input.slice(caret);
    }
    await loadDir(next.length > 0 ? next[next.length - 1] : undefined);
  }

  // Path-segment autocomplete: when the user types `@dir/fragment` in the file menu,
  // automatically descend into `dir` (if it matches a real entry) and filter by `fragment`.
  // Guard: only descend when the leading segment(s) unambiguously match a directory entry;
  // never descend mid-composition if already in that directory (prevents re-entry loops).
  $effect(() => {
    if (menu !== "file" || !query.includes("/")) return;
    const slashIdx = query.indexOf("/");
    const leadingName = query.slice(0, slashIdx);
    if (!leadingName) return;
    // Only descend if we're at root or the current dir doesn't already match the leading name.
    const currentTop = dirStack.length > 0 ? dirStack[dirStack.length - 1] : null;
    if (currentTop !== null) {
      const currentName = currentTop.split("/").filter(Boolean).pop() ?? "";
      if (currentName === leadingName) return; // already descended here
    }
    // Look for an exact (case-insensitive) directory match in the current listing.
    const match = entries.find(
      (e) => e.isDir && e.name.toLowerCase() === leadingName.toLowerCase(),
    );
    if (!match) return; // no match — just let filteredEntries handle the filter
    // Descend: update dirStack and reload. Clear the typed fragment so the input stays clean.
    const targetPath = match.path;
    dirStack = [...dirStack, targetPath];
    const caret = textarea ? textarea.selectionStart : input.length;
    if (tagStart >= 0 && caret > tagStart + 1) {
      input = input.slice(0, tagStart + 1) + input.slice(caret);
    }
    // Setting query to "" is what breaks the effect's re-entry cycle: the next run
    // hits the `!query.includes("/")` guard and returns before mutating state again.
    query = ""; // reset; syncMenu will re-derive the trailing fragment on next input
    active = 0;
    // loadDir is guarded by a sequence counter: if another call starts before this
    // one resolves, the stale response is discarded and entries is not overwritten.
    void loadDir(targetPath);
  });

  function openSlashMenu() {
    menu = "slash";
    active = 0;
    query = "";
    tagStart = -1;
  }

  async function openFileMenu(at: number) {
    menu = "file";
    active = 0;
    tagStart = at;
    query = "";
    dirStack = [];
    await loadDir();
  }

  async function openSkillMenu() {
    menu = "skill";
    active = 0;
    query = "";
    skillQuery = "";
    try {
      skills = await ipc.listSkills();
    } catch (e) {
      toast.error(`${e}`);
      closeMenu();
    }
  }

  // Open the skills editor dialog from the /skill menu. Closes the picker; the
  // $effect below re-lists skills once the dialog closes so edits show up.
  function manageSkills() {
    closeMenu();
    openSkillsEditor();
  }

  // When the skills editor closes, refresh the cached skill list so the picker
  // reflects any create/edit/rename/delete. Track the previous open state so we
  // only refresh on the closing edge.
  let skillsDialogWasOpen = false;
  $effect(() => {
    const isOpen = skillsStore.open;
    if (skillsDialogWasOpen && !isOpen) {
      ipc.listSkills().then((s) => (skills = s)).catch(() => {});
    }
    skillsDialogWasOpen = isOpen;
  });

  // Find the `@` that opens a file tag at the caret: the nearest `@` before the
  // caret with no whitespace between it and the caret. Returns -1 if none.
  function fileTagStart(before: string): number {
    const at = before.lastIndexOf("@");
    if (at < 0) return -1;
    // `@` must start a token (be at line start or preceded by whitespace).
    const prev = before[at - 1];
    if (at > 0 && prev !== undefined && !/\s/.test(prev)) return -1;
    if (/\s/.test(before.slice(at + 1))) return -1; // whitespace closes the tag
    return at;
  }

  // Inspect the caret context after each input/selection change and decide
  // whether to open, update, or close a menu.
  function syncMenu() {
    const el = textarea;
    const caret = el ? el.selectionStart : input.length;
    const before = input.slice(0, caret);

    // Skill menu: already open — keep skillQuery in sync with whatever the user
    // types. skillQuery tracks only the filter fragment, NOT the full composer
    // value, so prior lines don't pollute the filter. We derive the trailing
    // fragment as whatever was typed after the menu opened (the composer content
    // since the skill picker was launched). The simplest invariant: the composer
    // was empty (or cleared by runCommand) when the skill menu opened, so the
    // full current input IS the filter fragment. Guard this branch BEFORE the
    // slash-regex check so that typing characters while the skill menu is open
    // doesn't accidentally trigger openSlashMenu() and destroy the picker.
    if (menu === "skill") {
      // Derive the trailing fragment after the last line-start to exclude any
      // accidental multi-line composer content from poisoning the filter.
      const lineStart = before.lastIndexOf("\n") + 1;
      skillQuery = before.slice(lineStart);
      active = 0;
      return;
    }

    // Slash menu: `/` as the first char of a line (e.g. an empty composer).
    const lineStart = before.lastIndexOf("\n") + 1;
    const lineToCaret = before.slice(lineStart);
    if (/^\/[^\s]*$/.test(lineToCaret)) {
      if (menu !== "slash") openSlashMenu();
      query = lineToCaret.slice(1);
      active = 0;
      return;
    }
    if (menu === "slash") {
      closeMenu();
      return;
    }

    // File menu: an `@` token before the caret. Open on first sight, then keep
    // the query in sync until the `@` is gone or a space closes the tag.
    const at = fileTagStart(before);
    if (at < 0) {
      if (menu === "file") closeMenu();
      return;
    }
    if (menu !== "file") {
      openFileMenu(at);
      return;
    }
    tagStart = at;
    query = before.slice(at + 1);
    active = 0;
  }

  function oninput() {
    syncMenu();
  }

  async function selectEntry(e: FileEntry) {
    if (e.isDir) {
      dirStack = [...dirStack, e.path];
      query = "";
      active = 0;
      // Drop the typed filter fragment so the @ token resolves cleanly later.
      const caret = textarea ? textarea.selectionStart : input.length;
      if (tagStart >= 0 && caret > tagStart + 1) {
        input = input.slice(0, tagStart + 1) + input.slice(caret);
      }
      await loadDir(e.path);
      return;
    }
    insertTag(e.path);
  }

  // Replace the `@<query>` fragment with a finished `@<path> ` token.
  function insertTag(path: string) {
    const caret = textarea ? textarea.selectionStart : input.length;
    const start = tagStart >= 0 ? tagStart : caret;
    const end = Math.max(start, caret);
    input = input.slice(0, start) + `@${path} ` + input.slice(end);
    closeMenu();
    queueFocus(start + path.length + 2);
  }

  function runCommand(cmd: SlashCommand) {
    // Strip the `/<query>` fragment from the current line before running.
    const caret = textarea ? textarea.selectionStart : input.length;
    const before = input.slice(0, caret);
    const lineStart = before.lastIndexOf("\n") + 1;
    input = input.slice(0, lineStart) + input.slice(caret);
    closeMenu();
    cmd.run();
    queueFocus(lineStart);
  }

  async function selectSkill(skill: SkillDto) {
    try {
      await ipc.setActiveSkill(skill.name);
      activeSkillName = skill.name;
    } catch (e) {
      toast.error(`${e}`);
    }
    closeMenu();
    queueMicrotask(() => textarea?.focus());
  }

  async function clearActiveSkill() {
    try {
      await ipc.setActiveSkill(null);
      activeSkillName = null;
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  function queueFocus(pos: number) {
    queueMicrotask(() => {
      const el = textarea;
      if (!el) return;
      el.focus();
      el.setSelectionRange(pos, pos);
    });
  }

  function chooseActive() {
    if (menu === "file") {
      const e = filteredEntries[active];
      if (e) selectEntry(e);
    } else if (menu === "slash") {
      const c = filteredCommands[active];
      if (c) runCommand(c);
    } else if (menu === "skill") {
      const s = filteredSkills[active];
      if (s) selectSkill(s);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (menu !== "none") {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        if (itemCount > 0) active = (active + 1) % itemCount;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        if (itemCount > 0) active = (active - 1 + itemCount) % itemCount;
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        chooseActive();
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        closeMenu();
        // Return focus to the composer so typing can resume immediately.
        queueMicrotask(() => textarea?.focus());
        return;
      }
      // Backspace with an empty query ascends one directory level in the file picker.
      if (menu === "file" && e.key === "Backspace" && query === "" && dirStack.length > 0) {
        e.preventDefault();
        ascendDir();
        return;
      }
    }

    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }
</script>

<form
  class="relative border-t border-border p-3 flex flex-col gap-2 {dragOver
    ? 'ring-2 ring-inset ring-primary/60'
    : ''}"
  onsubmit={(e) => {
    e.preventDefault();
    submit();
  }}
>
  {#if dragOver}
    <div
      class="pointer-events-none absolute inset-0 z-40 flex items-center justify-center bg-background/70 text-sm font-medium text-muted-foreground"
    >
      Drop files to attach
    </div>
  {/if}
  {#if pastes.length > 0 || attachments.length > 0}
    <div class="flex flex-wrap gap-1.5">
      {#each pastes as p (p.id)}
        <span
          class="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted/60 px-2 py-1 text-xs text-muted-foreground"
        >
          <PaperclipIcon class="size-3.5" />
          pasted {p.lines} {p.lines === 1 ? "line" : "lines"}
          <button
            type="button"
            onclick={() => removePaste(p.id)}
            aria-label="Remove pasted text"
            class="rounded hover:text-foreground"
          >
            <XIcon class="size-3.5" />
          </button>
        </span>
      {/each}
      {#each attachments as a (a.id)}
        <span
          class="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted/60 px-2 py-1 text-xs text-muted-foreground"
          title={a.path}
        >
          {#if a.isImage}
            <ImageIcon class="size-3.5" />
          {:else}
            <FileIcon class="size-3.5" />
          {/if}
          <span class="max-w-48 truncate font-mono">{a.name}</span>
          <button
            type="button"
            onclick={() => removeAttachment(a.id)}
            aria-label="Remove attachment"
            class="rounded hover:text-foreground"
          >
            <XIcon class="size-3.5" />
          </button>
        </span>
      {/each}
    </div>
  {/if}
  <div class="flex items-center gap-2">
    <button
      type="button"
      onclick={() => openSettings("context-sources")}
      title="Manage context sources"
      class="inline-flex items-center gap-1.5 rounded-md px-1.5 py-0.5 text-xs text-muted-foreground transition-colors hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <span class="text-primary" aria-hidden="true">◇</span>
      {contextLabel}
    </button>
    {#if activeSkillName}
      <span
        class="inline-flex items-center gap-1 rounded-md border border-border bg-muted/60 px-1.5 py-0.5 text-xs text-muted-foreground"
        aria-label="Active skill: {activeSkillName}"
      >
        <BookOpenIcon class="size-3" />
        skill: {activeSkillName}
        <button
          type="button"
          onclick={clearActiveSkill}
          aria-label="Clear active skill"
          class="rounded hover:text-foreground"
        >
          <XIcon class="size-3" />
        </button>
      </span>
    {/if}
    {#if attachments.length > 0}
      <span class="text-xs text-muted-foreground">
        {attachments.length} attached
      </span>
    {/if}
    {#if showGauge}
      <span
        class="ml-auto inline-flex items-center gap-1.5 text-xs text-muted-foreground"
        title={gaugeTitle}
      >
        <span
          class="h-1.5 w-12 overflow-hidden rounded-full bg-muted"
          role="progressbar"
          aria-label="Context window usage"
          aria-valuenow={gaugePct}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <span
            class="block h-full rounded-full transition-all {gaugePct >= 90
              ? 'bg-destructive'
              : 'bg-primary/70'}"
            style="width: {gaugePct}%"
          ></span>
        </span>
        <span class="tabular-nums">{gaugeLabel}</span>
      </span>
    {/if}
  </div>
  <div class="flex items-end gap-2">
    <div class="relative flex-1">
      {#if menu !== "none"}
        <div
          class="absolute bottom-full left-0 mb-1 z-50 w-full max-w-md overflow-hidden rounded-md border border-border bg-popover text-popover-foreground shadow-md"
          role="listbox"
        >
          {#if menu === "file"}
            <div
              class="flex items-center gap-1.5 px-2 py-1 text-xs text-muted-foreground border-b border-border"
            >
              <FolderIcon class="size-3.5 shrink-0" />
              <span class="truncate font-mono">{dirStack.length > 0 ? dirStack[dirStack.length - 1] : "Allowed roots"}</span>
            </div>
          {:else if menu === "skill"}
            <div
              class="flex items-center gap-1.5 px-2 py-1 text-xs text-muted-foreground border-b border-border"
            >
              <BookOpenIcon class="size-3.5 shrink-0" />
              <span>Select a skill</span>
            </div>
          {/if}
          <div class="max-h-64 overflow-y-auto p-1">
            {#if menu === "file"}
              {#if loadingDir}
                <div class="flex items-center gap-2 px-2 py-1.5 text-sm text-muted-foreground">
                  <LoaderIcon class="size-4 shrink-0 animate-spin" />
                  Loading…
                </div>
              {:else}
              {#each filteredEntries as e, i (e.path)}
                <button
                  type="button"
                  role="option"
                  aria-selected={i === active}
                  class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm outline-hidden {i ===
                  active
                    ? 'bg-accent text-accent-foreground'
                    : ''}"
                  onmousedown={(ev) => {
                    ev.preventDefault();
                    selectEntry(e);
                  }}
                  onmousemove={() => (active = i)}
                >
                  {#if e.isDir}
                    <FolderIcon class="size-4 shrink-0 text-muted-foreground" />
                  {:else}
                    <FileIcon class="size-4 shrink-0 text-muted-foreground" />
                  {/if}
                  <FileListItem name={e.name} path={e.path} isDir={e.isDir} />
                </button>
              {:else}
                <div class="px-2 py-1.5 text-sm text-muted-foreground">No matches</div>
              {/each}
              {/if}
            {:else if menu === "skill"}
              {#each filteredSkills as s, i (s.name)}
                <button
                  type="button"
                  role="option"
                  aria-selected={i === active}
                  class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm outline-hidden {i ===
                  active
                    ? 'bg-accent text-accent-foreground'
                    : ''}"
                  onmousedown={(ev) => {
                    ev.preventDefault();
                    selectSkill(s);
                  }}
                  onmousemove={() => (active = i)}
                >
                  <BookOpenIcon class="size-4 shrink-0 text-muted-foreground" />
                  <span class="font-mono font-medium">{s.name}</span>
                  <span class="ml-auto truncate text-xs text-muted-foreground">{s.preview}</span>
                </button>
              {:else}
                <div class="px-2 py-1.5 text-sm text-muted-foreground">No skills found</div>
              {/each}
              <!-- Footer: open the authoring dialog. Separated from the pickable
                   skills so it's clearly an action, not a selectable skill. -->
              <button
                type="button"
                onmousedown={(ev) => {
                  ev.preventDefault();
                  manageSkills();
                }}
                class="mt-1 flex w-full items-center gap-2 rounded-sm border-t border-border px-2 py-1.5 text-left text-sm text-muted-foreground outline-hidden hover:text-foreground"
              >
                <PencilIcon class="size-4 shrink-0" />
                Manage skills…
              </button>
            {:else}
              {#each filteredCommands as c, i (c.name)}
                <button
                  type="button"
                  role="option"
                  aria-selected={i === active}
                  class="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm outline-hidden {i ===
                  active
                    ? 'bg-accent text-accent-foreground'
                    : ''}"
                  onmousedown={(ev) => {
                    ev.preventDefault();
                    runCommand(c);
                  }}
                  onmousemove={() => (active = i)}
                >
                  <TerminalIcon class="size-4 shrink-0 text-muted-foreground" />
                  <span class="font-mono font-medium">/{c.name}</span>
                  <span class="ml-auto truncate text-xs text-muted-foreground">{c.hint}</span>
                </button>
              {:else}
                <div class="px-2 py-1.5 text-sm text-muted-foreground">No matches</div>
              {/each}
            {/if}
          </div>
        </div>
      {/if}
      <Textarea
        bind:value={input}
        bind:ref={textarea}
        {onkeydown}
        {onpaste}
        {oninput}
        onblur={closeMenu}
        rows={2}
        placeholder={sessionStore.queue.length > 0
          ? "Message queued — sent when the turn finishes"
          : appStore.activeId
            ? `Ask ${appStore.activeId}…`
            : "Message zanto…"}
        class="resize-none"
      />
    </div>
    <Button
      type="button"
      size="icon"
      variant="ghost"
      onclick={pickFiles}
      aria-label="Attach files"
      title="Attach files"
    >
      <PaperclipIcon class="size-4" />
    </Button>
    {#if sessionStore.busy}
      <Button type="button" size="icon" variant="secondary" onclick={interrupt} aria-label="Stop">
        <SquareIcon class="size-4" />
      </Button>
    {:else}
      <Button type="submit" size="icon">
        <SendIcon class="size-4" />
      </Button>
    {/if}
  </div>
</form>
