// Session store: session list (for the active app), the active session's chat
// thread (segment-modeled entries), and the right-panel canvas block.
import { toast } from "svelte-sonner";
import { ipc, type ChatBlock, type RenderMsg, type SessionMeta } from "$lib/ipc";
import { activeApp } from "$lib/stores/app.svelte";

// A chat entry is a sequence of typed segments rather than a single block, so
// thinking/tool-call/component renderers are independent segment components.
export type ChatSegment =
  | { kind: "text"; text: string }
  | { kind: "reasoning"; text: string }
  | { kind: "tool_call"; id: string; name: string; args: any; output?: string; ok?: boolean }
  | { kind: "block"; block: ChatBlock }
  | { kind: "error"; message: string; retryText: string };

export type ChatEntry = { id: number; role: "user" | "assistant"; segments: ChatSegment[] };

// Monotonic id for stable {#each} keying. Entry ids must survive both streaming
// (segment-by-segment object replacement) and loadOlder() prepends, so keying by
// array index is wrong; every entry gets a unique id at creation and keeps it.
let nextEntryId = 0;
function entry(role: "user" | "assistant", segments: ChatSegment[]): ChatEntry {
  return { id: nextEntryId++, role, segments };
}

export const sessionStore = $state({
  sessions: [] as SessionMeta[],
  archivedSessions: [] as SessionMeta[], // archived sessions for the active app
  activeSessionId: null as string | null,
  convo: [] as ChatEntry[], // chat thread (role-tagged segment entries)
  canvas: null as ChatBlock | null, // right-panel view
  promotedLink: null as string | null, // a link promoted to the canvas panel
  busy: false,
  streaming: false, // assistant tokens currently arriving
  hasMore: false, // older history exists above the loaded window
  loadingOlder: false, // a loadOlder() fetch is in flight
});

// How many display messages to show on first open / fetch per scrollback page.
const PAGE_SIZE = 30;
// Offset (into the filtered display list) of the oldest message currently in
// `convo`. Older pages live at [loadedOffset - PAGE_SIZE, loadedOffset).
let loadedOffset = 0;

// Index of the live assistant entry currently being streamed (or null when the
// next streamed segment should open a fresh assistant entry).
let streamIdx: number | null = null;

/** Ensure a live assistant entry exists and return its index. */
function ensureLiveEntry(): number {
  if (streamIdx === null) {
    sessionStore.convo.push(entry("assistant", []));
    streamIdx = sessionStore.convo.length - 1;
  }
  return streamIdx;
}

/** Replace an entry's segments (reassign for reactivity). */
function setSegments(idx: number, segments: ChatSegment[]) {
  sessionStore.convo[idx] = { ...sessionStore.convo[idx], segments };
}

/** Wire the streaming turn events to the thread. Call once at app startup. */
export function initStreaming() {
  ipc.onChatChunk((text) => {
    const idx = ensureLiveEntry();
    const segs = [...sessionStore.convo[idx].segments];
    const last = segs[segs.length - 1];
    if (last && last.kind === "text") segs[segs.length - 1] = { kind: "text", text: last.text + text };
    else segs.push({ kind: "text", text });
    setSegments(idx, segs);
    sessionStore.streaming = true;
  });

  ipc.onChatReasoning((text) => {
    const idx = ensureLiveEntry();
    const segs = [...sessionStore.convo[idx].segments];
    const last = segs[segs.length - 1];
    if (last && last.kind === "reasoning")
      segs[segs.length - 1] = { kind: "reasoning", text: last.text + text };
    else segs.push({ kind: "reasoning", text });
    setSegments(idx, segs);
    sessionStore.streaming = true;
  });

  ipc.onChatToolCall((call) => {
    // A tool call closes any open text/reasoning segment.
    const idx = ensureLiveEntry();
    const segs = [...sessionStore.convo[idx].segments];
    segs.push({ kind: "tool_call", id: call.id, name: call.name, args: call.args });
    setSegments(idx, segs);
    sessionStore.streaming = true;
  });

  ipc.onChatToolResult((result) => {
    // Match the tool_call segment by id and fill in its output/outcome.
    if (streamIdx === null) return;
    const segs = [...sessionStore.convo[streamIdx].segments];
    const pos = segs.findIndex((s) => s.kind === "tool_call" && s.id === result.id);
    if (pos === -1) return;
    const s = segs[pos];
    if (s.kind === "tool_call") segs[pos] = { ...s, output: result.output, ok: result.ok };
    setSegments(streamIdx, segs);
  });

  ipc.onChatBlock((block) => {
    // Canvas blocks go to the right panel; inline blocks become a block segment.
    if (block.kind === "component" && block.target === "canvas") {
      sessionStore.canvas = block;
      return;
    }
    const idx = ensureLiveEntry();
    const segs = [...sessionStore.convo[idx].segments];
    segs.push({ kind: "block", block });
    setSegments(idx, segs);
  });

  ipc.onChatDone(() => {
    streamIdx = null;
    sessionStore.streaming = false;
  });
}

/** Refresh the session list for the active app. */
export async function loadSessions() {
  sessionStore.sessions = await ipc.listSessions();
}

/** Refresh the archived-session list for the active app. */
export async function loadArchived() {
  sessionStore.archivedSessions = await ipc.listArchivedSessions();
}

export async function newSession() {
  try {
    sessionStore.activeSessionId = await ipc.newSession();
    sessionStore.canvas = null;
    loadedOffset = 0;
    sessionStore.hasMore = false;
    sessionStore.loadingOlder = false;
    // Seed the chat-start NBA from the active app's suggested actions.
    const app = activeApp();
    sessionStore.convo =
      app && app.start_actions.length > 0
        ? [
            entry("assistant", [
              {
                kind: "block",
                block: {
                  kind: "component",
                  component_id: "nba",
                  data: { title: `${app.name} — quick actions`, actions: app.start_actions },
                  target: "inline",
                },
              },
            ]),
          ]
        : [];
    await loadSessions();
    await loadArchived();
  } catch (e) {
    toast.error(`${e}`);
  }
}

function toEntries(msgs: RenderMsg[]): ChatEntry[] {
  return msgs.map((m) => {
    const segments: ChatSegment[] = [];
    // A blocks-only turn has empty text; skip the empty bubble in that case.
    if (m.text.trim() !== "") segments.push({ kind: "text", text: m.text });
    // Restore persisted component blocks (D1) as block segments after the text.
    // Canvas-targeted blocks render inline on reload — acceptable.
    for (const block of m.blocks?.blocks ?? []) segments.push({ kind: "block", block });
    return entry(m.role, segments);
  });
}

export async function selectSession(id: string) {
  try {
    // Full load sets the active session in core state and gives the total count;
    // we only render the most-recent page, then page older on scrollback.
    const all = await ipc.loadSession(id);
    const total = all.length;
    const start = Math.max(0, total - PAGE_SIZE);
    sessionStore.convo = toEntries(all.slice(start));
    loadedOffset = start;
    sessionStore.hasMore = start > 0;
    sessionStore.loadingOlder = false;
    sessionStore.canvas = null;
    sessionStore.activeSessionId = id;
  } catch (e) {
    toast.error(`${e}`);
  }
}

/** Fetch the previous page of history and PREPEND it to `convo`. */
export async function loadOlder() {
  const id = sessionStore.activeSessionId;
  if (!id || !sessionStore.hasMore || sessionStore.loadingOlder) return;
  sessionStore.loadingOlder = true;
  try {
    const offset = Math.max(0, loadedOffset - PAGE_SIZE);
    const limit = loadedOffset - offset;
    const older = await ipc.loadSessionPage(id, offset, limit);
    sessionStore.convo = [...toEntries(older), ...sessionStore.convo];
    loadedOffset = offset;
    sessionStore.hasMore = offset > 0;
  } catch (e) {
    toast.error(`${e}`);
  } finally {
    sessionStore.loadingOlder = false;
  }
}

export async function renameSession(id: string, title: string) {
  try {
    await ipc.renameSession(id, title);
    await loadSessions();
  } catch (e) {
    toast.error(`${e}`);
  }
}

export async function deleteSession(id: string) {
  try {
    await ipc.deleteSession(id);
    if (sessionStore.activeSessionId === id) await newSession();
    else await loadSessions();
    await loadArchived();
  } catch (e) {
    toast.error(`${e}`);
  }
}

/** Archive a session: move it out of the active list. If it's the open one,
 * start a fresh session so the thread doesn't reference an archived session. */
export async function archiveSession(id: string) {
  try {
    await ipc.archiveSession(id);
    if (sessionStore.activeSessionId === id) await newSession();
    else await loadSessions();
    await loadArchived();
  } catch (e) {
    toast.error(`${e}`);
  }
}

/** Unarchive a session: restore it to the active list. */
export async function unarchiveSession(id: string) {
  try {
    await ipc.unarchiveSession(id);
    await loadSessions();
    await loadArchived();
  } catch (e) {
    toast.error(`${e}`);
  }
}

/**
 * Send a chat turn. The thread is assembled live from streaming events
 * (`chat_chunk`/`chat_reasoning`/`chat_tool_call`/`chat_tool_result`/`chat_block`/
 * `chat_done` via {@link initStreaming}); the awaited return is authoritative but
 * is not re-rendered to avoid duplication.
 */
export async function send(text: string): Promise<void> {
  sessionStore.convo.push(entry("user", [{ kind: "text", text }]));
  sessionStore.busy = true;
  streamIdx = null;
  try {
    await ipc.sendMessage(text);
  } catch (e) {
    // Surface the failed turn inline so it can be retried, not just a toast.
    const message = `${e}`;
    toast.error(message);
    sessionStore.convo.push(entry("assistant", [{ kind: "error", message, retryText: text }]));
  } finally {
    sessionStore.busy = false;
    sessionStore.streaming = false;
    streamIdx = null;
    await loadSessions(); // titles/timestamps may have changed
  }
}

/**
 * Retry a failed turn. Strips the trailing failed-turn entries — the error
 * entry, any partial assistant entry produced before the stream rejected, and
 * the original user entry — then re-sends (which re-adds the user entry once).
 * Without dropping the user entry too, send() would push a duplicate user
 * bubble on every retry.
 */
export async function retry(text: string): Promise<void> {
  const convo = sessionStore.convo;
  const last = convo[convo.length - 1];
  if (!last || !last.segments.some((s) => s.kind === "error")) {
    // Trailing entry isn't a live error bubble; don't disturb the thread.
    await send(text);
    return;
  }
  // Walk back over the failed-turn entries up to and including its user entry.
  let cut = convo.length - 1; // the error entry
  while (cut > 0 && convo[cut - 1].role !== "user") cut--;
  if (cut > 0) cut--; // include the user entry
  sessionStore.convo = convo.slice(0, cut);
  await send(text);
}
