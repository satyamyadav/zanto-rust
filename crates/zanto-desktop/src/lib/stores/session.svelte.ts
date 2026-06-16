// Session store: session list (for the active app), the active session's chat
// thread (blocks), and the right-panel canvas block.
import { ipc, type ChatBlock, type SessionMeta } from "$lib/ipc";

export type ChatEntry = { role: "user" | "assistant"; block: ChatBlock };

export const sessionStore = $state({
  sessions: [] as SessionMeta[],
  convo: [] as ChatEntry[], // chat thread (role-tagged blocks)
  canvas: null as ChatBlock | null, // right-panel view
  busy: false,
});

/** Refresh the session list for the active app. */
export async function loadSessions() {
  sessionStore.sessions = await ipc.listSessions();
}

export async function newSession() {
  await ipc.newSession();
  sessionStore.convo = [];
  sessionStore.canvas = null;
  await loadSessions();
}

export async function selectSession(id: string) {
  await ipc.loadSession(id);
  // History is on the backend; the thread shows fresh from here (no message
  // backfill in this slice). Clear the visible thread.
  sessionStore.convo = [];
}

export async function renameSession(id: string, title: string) {
  await ipc.renameSession(id, title);
  await loadSessions();
}

export async function deleteSession(id: string) {
  await ipc.deleteSession(id);
  await loadSessions();
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
