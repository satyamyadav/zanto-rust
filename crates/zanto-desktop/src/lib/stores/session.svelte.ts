// Session store: session list (for the active app), the active session's chat
// thread (segment-modeled entries), and the right-panel canvas block.
import { toast } from "svelte-sonner";
import { ipc, type ChatBlock, type RenderMsg, type SessionMeta } from "$lib/ipc";
import { activeApp, appStore } from "$lib/stores/app.svelte";

// A chat entry is a sequence of typed segments rather than a single block, so
// thinking/tool-call/component renderers are independent segment components.
export type ChatSegment =
  | { kind: "text"; text: string }
  | { kind: "reasoning"; text: string }
  | {
      kind: "tool_call";
      id: string;
      name: string;
      args: any;
      output?: string;
      ok?: boolean;
      // True when this tool's result rendered as an inline block — its tool-call
      // card is hidden by this flag, not by matching tool names (B5-1).
      renders_as_block?: boolean;
    }
  | { kind: "block"; block: ChatBlock }
  | { kind: "error"; message: string; retryText: string };

export type ChatAttachment = {
  path: string;
  name: string;
  isImage: boolean;
};

export type ChatEntry = {
  id: number;
  role: "user" | "assistant";
  segments: ChatSegment[];
  stopped?: boolean; // true when this assistant turn was interrupted (Stop)
  attachments?: ChatAttachment[];
};

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
  canvas: null as ChatBlock | null, // right-panel view (agent component block)
  promotedLink: null as string | null, // a link promoted to the canvas panel
  panelMode: null as "browser" | null, // non-block panel view (artifact browser)
  queue: [] as string[], // messages typed while busy, dispatched FIFO when free
  busy: false,
  streaming: false, // assistant tokens currently arriving
  contextSummarized: false, // older turns folded into the summary to fit the window
  hasMore: false, // older history exists above the loaded window
  loadingOlder: false, // a loadOlder() fetch is in flight
  sessionsHasMore: false, // more session-list pages exist below the loaded window
  loadingMoreSessions: false, // a loadMoreSessions() fetch is in flight
});

// How many display messages to show on first open / fetch per scrollback page.
const PAGE_SIZE = 30;
// Offset (into the filtered display list) of the oldest message currently in
// `convo`. Older pages live at [loadedOffset - PAGE_SIZE, loadedOffset).
let loadedOffset = 0;

// How many sessions to fetch per session-list page (infinite scroll).
const SESSIONS_PAGE_SIZE = 30;
// Bumped on every full session-list refresh / app switch / new session. A
// loadMoreSessions() in flight captures it and discards its page if the
// generation moved, so a stale append can't land on a freshly-refreshed list.
let sessionsLoadGen = 0;

// Index of the live assistant entry currently being streamed (or null when the
// next streamed segment should open a fresh assistant entry).
let streamIdx: number | null = null;

/** Reset the live-turn state when switching sessions. The interrupted turn's own
 * finally is guarded to no-op once the active session changed, so the new session
 * must clear busy/streaming/streamIdx itself to start clean. */
function resetLiveTurn() {
  sessionStore.busy = false;
  sessionStore.streaming = false;
  sessionStore.contextSummarized = false;
  streamIdx = null;
}

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
      // An agent canvas block takes the panel; clear the sibling panel views so
      // they don't shadow it (Canvas precedence: link > browser > canvas block).
      sessionStore.promotedLink = null;
      sessionStore.panelMode = null;
      sessionStore.canvas = block;
      return;
    }
    const idx = ensureLiveEntry();
    const segs = [...sessionStore.convo[idx].segments];
    // An inline block is produced by the most recent in-flight tool call (the
    // dispatch emits the block between the tool_call and its result). Flag that
    // tool call so its card is hidden authoritatively (B5-1).
    for (let k = segs.length - 1; k >= 0; k--) {
      const s = segs[k];
      if (s.kind === "tool_call") {
        segs[k] = { ...s, renders_as_block: true };
        break;
      }
    }
    segs.push({ kind: "block", block });
    setSegments(idx, segs);
  });

  ipc.onChatStopped(() => {
    // Mark the live assistant entry as stopped so MessageList shows the marker.
    // Arrives before chat_done (which clears streamIdx), so the live entry — if the
    // turn produced any output — is still addressable. When the turn was stopped
    // before emitting anything (streamIdx null), there's no content to mark; the
    // vanished thinking indicator is feedback enough, so don't spawn an empty bubble.
    if (streamIdx === null) return;
    sessionStore.convo[streamIdx] = { ...sessionStore.convo[streamIdx], stopped: true };
  });

  ipc.onChatSummarized(() => {
    // Older history was folded into the running summary to fit the model's context
    // window — surface the divider so the compaction is visible (B6 / CO-2).
    sessionStore.contextSummarized = true;
  });

  ipc.onChatDone(() => {
    streamIdx = null;
    sessionStore.streaming = false;
  });
}

/** Refresh the session list for the active app, preserving how many rows are
 * currently shown so an infinite-scrolled list isn't collapsed back to one page
 * by an unrelated refresh (rename/delete/turn-finish all call this). Bumps the
 * load generation, which invalidates any loadMoreSessions() in flight.
 *
 * Captures the active app id at call time and only commits the result if that
 * app is still active when the IPC resolves, so a slow load from app A cannot
 * overwrite app B's list after a switch. */
export async function loadSessions() {
  const appId = appStore.activeId;
  ++sessionsLoadGen; // invalidate any loadMoreSessions() in flight
  // Re-fetch at least one page, or enough to cover everything already scrolled
  // into view (rounded up to a whole page), so the visible window is preserved.
  const want = Math.max(SESSIONS_PAGE_SIZE, ceilToPage(sessionStore.sessions.length));
  const page = await ipc.listSessionsPage(0, want);
  if (appStore.activeId !== appId) return; // stale: app switched mid-load
  sessionStore.sessions = page;
  sessionStore.sessionsHasMore = page.length === want;
  sessionStore.loadingMoreSessions = false;
}

/** Append the next page of sessions (infinite scroll). Guarded against both an
 * app switch and a concurrent full refresh (generation bump) landing first. */
export async function loadMoreSessions() {
  if (!sessionStore.sessionsHasMore || sessionStore.loadingMoreSessions) return;
  const appId = appStore.activeId;
  const gen = sessionsLoadGen;
  sessionStore.loadingMoreSessions = true;
  try {
    const offset = sessionStore.sessions.length;
    const page = await ipc.listSessionsPage(offset, SESSIONS_PAGE_SIZE);
    // Discard the page if the list was refreshed or the app switched mid-fetch,
    // so it can't append onto a list it no longer lines up with.
    if (appStore.activeId !== appId || sessionsLoadGen !== gen) return;
    sessionStore.sessions = [...sessionStore.sessions, ...page];
    sessionStore.sessionsHasMore = page.length === SESSIONS_PAGE_SIZE;
  } catch (e) {
    toast.error(`${e}`);
  } finally {
    if (appStore.activeId === appId && sessionsLoadGen === gen) {
      sessionStore.loadingMoreSessions = false;
    }
  }
}

/** Round a count up to a whole number of session pages. */
function ceilToPage(n: number): number {
  return Math.ceil(n / SESSIONS_PAGE_SIZE) * SESSIONS_PAGE_SIZE;
}

/** Refresh the archived-session list for the active app. Active-id guarded. */
export async function loadArchived() {
  const appId = appStore.activeId;
  const list = await ipc.listArchivedSessions();
  if (appStore.activeId !== appId) return; // stale: app switched mid-load
  sessionStore.archivedSessions = list;
}

export async function newSession() {
  try {
    // Switching away from a live turn: stop it and drop any pending queued messages
    // so they don't dispatch into the new session.
    if (sessionStore.busy) await interrupt();
    resetLiveTurn();
    sessionStore.queue = [];
    sessionStore.activeSessionId = await ipc.newSession();
    sessionStore.canvas = null;
    sessionStore.promotedLink = null;
    sessionStore.panelMode = null;
    loadedOffset = 0;
    sessionStore.hasMore = false;
    sessionStore.loadingOlder = false;
    // Reset session-list paging: a new session / app switch starts the list at
    // page 0 (clearing the window also stops loadSessions from re-fetching the
    // previous app's larger scrolled window).
    sessionStore.sessions = [];
    sessionStore.sessionsHasMore = false;
    sessionStore.loadingMoreSessions = false;
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
    // Preferred path: the turn persisted its full ordered display segments
    // (reasoning/tool_call/block/text). Rebuild them in order so the reopened
    // turn looks identical to how it streamed — the reasoning Thinking block,
    // inline tool calls, artifacts, and text. `stopped` restores the marker.
    if (m.segments && m.segments.length > 0) {
      const segments = m.segments
        .map((s): ChatSegment | null => {
          switch (s.kind) {
            case "text":
              return { kind: "text", text: s.text };
            case "reasoning":
              return { kind: "reasoning", text: s.text };
            case "tool_call":
              return { kind: "tool_call", id: s.id, name: s.name, args: s.args, output: s.output, ok: s.ok, renders_as_block: s.renders_as_block };
            case "block":
              return { kind: "block", block: s.block };
            default:
              // Forward-incompatible / corrupted blob: drop the unknown segment
              // rather than emit an undefined entry that crashes rendering.
              return null;
          }
        })
        .filter((s): s is ChatSegment => s !== null);
      const e = entry(m.role, segments);
      if (m.stopped) e.stopped = true;
      if (m.attachments && m.attachments.length > 0) {
        e.attachments = m.attachments.map((a) => ({ path: a.path, name: a.name, isImage: a.is_image }));
      }
      return e;
    }
    // Back-compat: legacy sessions without persisted segments — rebuild from
    // text + persisted component blocks.
    const segments: ChatSegment[] = [];
    // A blocks-only turn has empty text; skip the empty bubble in that case.
    if (m.text.trim() !== "") segments.push({ kind: "text", text: m.text });
    // Restore persisted component blocks (D1) as block segments after the text.
    // Canvas-targeted blocks render inline on reload — acceptable.
    for (const block of m.blocks?.blocks ?? []) segments.push({ kind: "block", block });
    const e = entry(m.role, segments);
    if (m.stopped) e.stopped = true;
    if (m.attachments && m.attachments.length > 0) {
      e.attachments = m.attachments.map((a) => ({ path: a.path, name: a.name, isImage: a.is_image }));
    }
    return e;
  });
}

export async function selectSession(id: string) {
  try {
    // Switching away from a live turn: stop it and drop pending queued messages.
    if (sessionStore.busy) await interrupt();
    resetLiveTurn();
    sessionStore.queue = [];
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
    sessionStore.promotedLink = null;
    sessionStore.panelMode = null;
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
export async function send(text: string, imagePaths: string[] = [], attachments: ChatAttachment[] = []): Promise<void> {
  // Busy: queue the message (FIFO) and return without invoking. The running turn's
  // `finally` dispatches the next queued message when it frees up. The queue is
  // text-only; image attachments queued mid-turn are not carried (a rare edge —
  // attaching images while a turn is already running).
  if (sessionStore.busy) {
    sessionStore.queue.push(text);
    return;
  }
  // Snapshot the session this turn belongs to. If the user switches sessions while
  // the turn is in flight (interrupt + select/new), this turn's late-resolving
  // promise must NOT touch the new session's shared turn state or queue.
  const turnSessionId = sessionStore.activeSessionId;
  const userEntry = entry("user", [{ kind: "text", text }]);
  if (attachments.length > 0) userEntry.attachments = attachments;
  sessionStore.convo.push(userEntry);
  sessionStore.busy = true;
  streamIdx = null;
  try {
    await ipc.sendMessage(text, imagePaths);
  } catch (e) {
    // The session was switched mid-turn; the new session owns the thread now.
    if (sessionStore.activeSessionId !== turnSessionId) return;
    // Surface the failed turn inline so it can be retried, not just a toast.
    const message = `${e}`;
    toast.error(message);
    sessionStore.convo.push(entry("assistant", [{ kind: "error", message, retryText: text }]));
  } finally {
    // Only reconcile shared state / dispatch the queue when still on this session.
    // After a mid-turn switch, a new turn may already own busy/streaming/streamIdx
    // and the queue belongs to the new session — leave them untouched.
    if (sessionStore.activeSessionId === turnSessionId) {
      sessionStore.busy = false;
      sessionStore.streaming = false;
      streamIdx = null;
      await loadSessions(); // titles/timestamps may have changed
      // Dispatch the next queued message, if any (FIFO; recursion is fine).
      if (sessionStore.queue.length > 0) {
        const next = sessionStore.queue.shift()!;
        void send(next);
      }
    }
  }
}

/** Interrupt the in-flight turn. The running send()'s promise then resolves (core
 * returns the partial), its `finally` runs, and the next queued message dispatches. */
export async function interrupt(): Promise<void> {
  try {
    await ipc.interruptTurn();
  } catch (e) {
    toast.error(`${e}`);
  }
}

/** Remove a queued (not-yet-sent) message by index. */
export function removeQueued(i: number) {
  sessionStore.queue.splice(i, 1);
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
