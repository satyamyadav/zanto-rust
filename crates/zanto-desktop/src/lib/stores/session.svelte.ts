// Session store: session list (for the active app), the active session's chat
// thread (segment-modeled entries), and the right-panel canvas block.
import { toast } from "svelte-sonner";
import { ipc, type ChatBlock, type SessionMeta } from "$lib/ipc";
import { activeApp } from "$lib/stores/app.svelte";

// A chat entry is a sequence of typed segments rather than a single block, so
// thinking/tool-call/component renderers are independent segment components.
export type ChatSegment =
  | { kind: "text"; text: string }
  | { kind: "reasoning"; text: string }
  | { kind: "tool_call"; id: string; name: string; args: any; output?: string; ok?: boolean }
  | { kind: "block"; block: ChatBlock };

export type ChatEntry = { role: "user" | "assistant"; segments: ChatSegment[] };

export const sessionStore = $state({
  sessions: [] as SessionMeta[],
  activeSessionId: null as string | null,
  convo: [] as ChatEntry[], // chat thread (role-tagged segment entries)
  canvas: null as ChatBlock | null, // right-panel view
  busy: false,
  streaming: false, // assistant tokens currently arriving
});

// Index of the live assistant entry currently being streamed (or null when the
// next streamed segment should open a fresh assistant entry).
let streamIdx: number | null = null;

/** Ensure a live assistant entry exists and return its index. */
function ensureLiveEntry(): number {
  if (streamIdx === null) {
    sessionStore.convo.push({ role: "assistant", segments: [] });
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

export async function newSession() {
  try {
    sessionStore.activeSessionId = await ipc.newSession();
    sessionStore.canvas = null;
    // Seed the chat-start NBA from the active app's suggested actions.
    const app = activeApp();
    sessionStore.convo =
      app && app.start_actions.length > 0
        ? [
            {
              role: "assistant",
              segments: [
                {
                  kind: "block",
                  block: {
                    kind: "component",
                    component_id: "nba",
                    data: { title: `${app.name} — quick actions`, actions: app.start_actions },
                    target: "inline",
                  },
                },
              ],
            },
          ]
        : [];
    await loadSessions();
  } catch (e) {
    toast.error(`${e}`);
  }
}

export async function selectSession(id: string) {
  try {
    const msgs = await ipc.loadSession(id);
    sessionStore.convo = msgs.map((m) => ({
      role: m.role,
      segments: [{ kind: "text", text: m.text }] as ChatSegment[],
    }));
    sessionStore.canvas = null;
    sessionStore.activeSessionId = id;
  } catch (e) {
    toast.error(`${e}`);
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
  sessionStore.convo.push({ role: "user", segments: [{ kind: "text", text }] });
  sessionStore.busy = true;
  streamIdx = null;
  try {
    await ipc.sendMessage(text);
  } finally {
    sessionStore.busy = false;
    sessionStore.streaming = false;
    streamIdx = null;
    await loadSessions(); // titles/timestamps may have changed
  }
}
