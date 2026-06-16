// Session store: session list (for the active app), the active session's chat
// thread (blocks), and the right-panel canvas block.
import { toast } from "svelte-sonner";
import { ipc, type ChatBlock, type SessionMeta } from "$lib/ipc";
import { activeApp } from "$lib/stores/app.svelte";

export type ChatEntry = { role: "user" | "assistant"; block: ChatBlock };

export const sessionStore = $state({
  sessions: [] as SessionMeta[],
  activeSessionId: null as string | null,
  convo: [] as ChatEntry[], // chat thread (role-tagged blocks)
  canvas: null as ChatBlock | null, // right-panel view
  busy: false,
  streaming: false, // assistant tokens currently arriving
});

// Index of the live assistant text entry currently being streamed (or null when
// the next text delta should open a fresh bubble).
let streamIdx: number | null = null;

/** Wire the streaming turn events to the thread. Call once at app startup. */
export function initStreaming() {
  ipc.onChatChunk((text) => {
    if (streamIdx === null) {
      sessionStore.convo.push({ role: "assistant", block: { kind: "markdown", text: "" } });
      streamIdx = sessionStore.convo.length - 1;
    }
    const e = sessionStore.convo[streamIdx];
    if (e.block.kind === "markdown") {
      // Reassign for reactivity.
      sessionStore.convo[streamIdx] = {
        ...e,
        block: { kind: "markdown", text: e.block.text + text },
      };
    }
    sessionStore.streaming = true;
  });

  ipc.onChatBlock((block) => {
    // Close the current text bubble; route the block to canvas or thread.
    streamIdx = null;
    if (block.kind === "component" && block.target === "canvas") sessionStore.canvas = block;
    else sessionStore.convo.push({ role: "assistant", block });
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
              block: {
                kind: "component",
                component_id: "nba",
                data: { title: `${app.name} — quick actions`, actions: app.start_actions },
                target: "inline",
              },
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
      block: { kind: "markdown", text: m.text } as ChatBlock,
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
 * (`chat_chunk`/`chat_block`/`chat_done` via {@link initStreaming}); the awaited
 * return is the authoritative turn but is not re-rendered to avoid duplication.
 */
export async function send(text: string): Promise<void> {
  sessionStore.convo.push({ role: "user", block: { kind: "markdown", text } });
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
