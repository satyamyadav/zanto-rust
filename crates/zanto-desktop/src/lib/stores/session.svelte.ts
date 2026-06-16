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
});

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

/** Send a chat turn; route inline blocks to the thread, canvas blocks to the panel. */
export async function send(text: string): Promise<void> {
  sessionStore.convo.push({ role: "user", block: { kind: "markdown", text } });
  sessionStore.busy = true;
  try {
    const turn = await ipc.sendMessage(text);
    for (const b of turn.blocks) {
      if (b.kind === "component" && b.target === "canvas") sessionStore.canvas = b;
      else sessionStore.convo.push({ role: "assistant", block: b });
    }
  } finally {
    sessionStore.busy = false;
    await loadSessions(); // titles/timestamps may have changed
  }
}
