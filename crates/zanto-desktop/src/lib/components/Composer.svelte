<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Textarea } from "$lib/components/ui/textarea";
  import { toast } from "svelte-sonner";
  import SendIcon from "@lucide/svelte/icons/send";
  import PaperclipIcon from "@lucide/svelte/icons/paperclip";
  import XIcon from "@lucide/svelte/icons/x";
  import FileIcon from "@lucide/svelte/icons/file";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import TerminalIcon from "@lucide/svelte/icons/terminal";
  import LoaderIcon from "@lucide/svelte/icons/loader";
  import { sessionStore, send, newSession } from "$lib/stores/session.svelte";
  import { appStore } from "$lib/stores/app.svelte";
  import { ipc, type FileEntry } from "$lib/ipc";

  // Large pastes become collapsed chips instead of flooding the textarea; the
  // full text is still spliced into the final message on send.
  const CHAR_THRESHOLD = 2000;
  const LINE_THRESHOLD = 20;

  type Paste = { id: number; text: string; lines: number };

  let input = $state("");
  let pastes = $state<Paste[]>([]);
  let nextId = 0;
  let textarea = $state<HTMLTextAreaElement | null>(null);

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
    const attached = pastes.map((p) => p.text).join("\n\n");
    return [typed, attached].filter((s) => s.length > 0).join("\n\n");
  }

  async function submit() {
    if (sessionStore.busy) return;
    const text = composeMessage();
    if (!text) return;
    input = "";
    pastes = [];
    closeMenu();
    try {
      await send(text);
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  // ── Composer menus (@ file-picker / slash commands) ──────────────────────
  // A single overlay drives both menus; `menu` selects which is active.
  type Menu = "none" | "file" | "slash";

  // Slash-command registry. Add entries here to extend the menu.
  type SlashCommand = { name: string; hint: string; run: () => void };
  const SLASH_COMMANDS: SlashCommand[] = [
    { name: "new", hint: "Start a new session", run: () => newSession() },
    { name: "clear", hint: "Clear the composer", run: clearInput },
  ];

  let menu = $state<Menu>("none");
  let active = $state(0); // highlighted item index
  // @-menu state: the directory listing, what's been typed after `@`, the
  // `@` position in `input`, and the current directory (null = roots).
  let entries = $state<FileEntry[]>([]);
  let dirStack = $state<string[]>([]); // breadcrumb of descended paths
  let tagStart = -1; // index of the `@` that opened the file menu
  let query = $state(""); // text typed after `@` (or `/`)
  let loadingDir = $state(false); // a directory listing fetch is in flight

  function clearInput() {
    input = "";
    pastes = [];
  }

  function closeMenu() {
    menu = "none";
    active = 0;
    query = "";
    tagStart = -1;
  }

  const filteredEntries = $derived(
    query
      ? entries.filter((e) => e.name.toLowerCase().includes(query.toLowerCase()))
      : entries,
  );
  const filteredCommands = $derived(
    query
      ? SLASH_COMMANDS.filter((c) => c.name.toLowerCase().includes(query.toLowerCase()))
      : SLASH_COMMANDS,
  );
  const itemCount = $derived(menu === "file" ? filteredEntries.length : filteredCommands.length);

  async function loadDir(path?: string) {
    loadingDir = true;
    try {
      entries = await ipc.browseDir(path);
    } catch (e) {
      toast.error(`${e}`);
      closeMenu();
    } finally {
      loadingDir = false;
    }
  }

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
    }

    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }
</script>

<form
  class="border-t border-border p-3 flex flex-col gap-2"
  onsubmit={(e) => {
    e.preventDefault();
    submit();
  }}
>
  {#if pastes.length > 0}
    <div class="flex flex-wrap gap-1.5">
      {#each pastes as p (p.id)}
        <span
          class="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted px-2 py-1 text-xs text-muted-foreground"
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
    </div>
  {/if}
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
                  <span class="truncate font-mono">{e.name}</span>
                </button>
              {:else}
                <div class="px-2 py-1.5 text-sm text-muted-foreground">No matches</div>
              {/each}
              {/if}
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
        placeholder={appStore.activeId ? `Ask ${appStore.activeId}…` : "Message zanto…"}
        class="resize-none"
      />
    </div>
    <Button type="submit" size="icon" disabled={sessionStore.busy}>
      <SendIcon class="size-4" />
    </Button>
  </div>
</form>
