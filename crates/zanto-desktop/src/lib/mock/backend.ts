import type {
  Config, AppManifest, ArtifactDef, SessionMeta, ChatTurn,
} from "$lib/ipc";
import { emit } from "./event";
import { defaultScenario, pickScenario } from "./scenarios";

import getConfigFx from "../../../contract/fixtures/get_config.json";
import listAppsFx from "../../../contract/fixtures/list_apps.json";
import getCatalogueFx from "../../../contract/fixtures/get_catalogue.json";
import listSessionsFx from "../../../contract/fixtures/list_sessions.json";
import newSessionFx from "../../../contract/fixtures/new_session.json";
import listPinnedFx from "../../../contract/fixtures/list_pinned_artifacts.json";
import loadSessionFx from "../../../contract/fixtures/load_session.json";

let interrupted = false;
let interruptResolve: (() => void) | null = null;
let errorArmed = true;
let pinned: any[] = listPinnedFx.response.slice();
let nextPinId = pinned.length + 1;

// Deterministic 60-entry history used by load_session_page (C-11 scrollback tests).
const longSession = Array.from({ length: 60 }, (_, i) => ({
  role: i % 2 === 0 ? "user" : "assistant",
  text: `msg #${i}`,
  blocks: null,
  segments: null,
  stopped: null,
}));

// Each handler is keyed by the exact `invoke` command name used in ipc.ts.
// Typed return values turn the fixture JSON into a compile-time contract.
export const backend: Record<string, (args: any) => Promise<unknown>> = {
  get_config: async (): Promise<Config> => getConfigFx.response as Config,
  list_apps: async (): Promise<AppManifest[]> => listAppsFx.response,
  get_catalogue: async (): Promise<ArtifactDef[]> => getCatalogueFx.response,
  list_sessions: async (): Promise<SessionMeta[]> => listSessionsFx.response,
  list_sessions_page: async (): Promise<SessionMeta[]> => [
    ...listSessionsFx.response,
    {
      id: "sess-long",
      title: "Long session",
      workspace: "/home/user/project",
      app_id: null,
      created_at: 1700000100,
      updated_at: 1700000200,
      message_count: 60,
      archived: false,
    } satisfies SessionMeta,
  ],
  list_archived_sessions: async (): Promise<SessionMeta[]> => listSessionsFx.response,
  new_session: async (): Promise<string> => newSessionFx.response,
  mount_app: async () => undefined,
  unmount_app: async () => undefined,
  send_message: async (args: { text?: string }): Promise<ChatTurn> => {
    interrupted = false;
    interruptResolve = null;
    const sc = pickScenario(args?.text ?? "");
    // One-shot error: first attempt throws; retry falls through to defaultScenario.
    if (sc.throws) {
      if (errorArmed) { errorArmed = false; throw new Error("mock: simulated turn failure"); }
      // recovered attempt: replay the default scenario
      for (const ev of defaultScenario.events) { emit(ev.event, ev.payload); await Promise.resolve(); }
      return defaultScenario.response;
    }
    for (const ev of sc.events) {
      if (interrupted) break;
      emit(ev.event, ev.payload);
      await Promise.resolve(); // yield a microtask so the UI updates between deltas
    }
    // Blocking scenarios (e.g. "silent stop") park here until interrupt_turn is
    // called — simulating a long-running turn the user stops early. Without this,
    // send_message would return immediately and sessionStore.busy would drop before
    // the Stop button renders.
    if (!interrupted && sc.blocking) {
      await new Promise<void>((resolve) => { interruptResolve = resolve; });
    }
    if (interrupted) { emit("chat_stopped", null); emit("chat_done", null); }
    return sc.response;
  },
  interrupt_turn: async () => {
    interrupted = true;
    interruptResolve?.();
    interruptResolve = null;
  },
  load_session: async (a: { id?: string }): Promise<any> =>
    a?.id === "sess-long" ? longSession : loadSessionFx.response,
  load_session_page: async (a: { offset?: number; limit?: number }): Promise<any> => {
    const offset = a?.offset ?? 0;
    const limit = a?.limit ?? 20;
    // offset is the absolute index of the first message to return (0-based, oldest-first).
    // This matches the store's loadOlder() call: offset = loadedOffset - PAGE_SIZE.
    return longSession.slice(offset, offset + limit);
  },
  list_pinned_artifacts: async (): Promise<any> => pinned,
  read_pinned_artifact: async (a: { id: number }): Promise<any> =>
    pinned.find((p) => p.id === a.id) ?? pinned[0],
  pin_artifact_cmd: async (a: { componentId: string; data: any; title?: string }): Promise<number> => {
    const id = nextPinId++;
    pinned.push({ id, component_id: a.componentId, title: a.title ?? null, target: "inline", created_at: 1718900000, data: a.data });
    return id;
  },
  query_app: async (): Promise<any> => ({ income: 2000, spent: 12.5, net: 1987.5, by_category: { dining: 12.5 } }),
  run_app_action: async (): Promise<any> => ({}),
  // Minimal seed so the @-tag autocomplete (C-8) has entries to display.
  browse_dir: async (): Promise<{ name: string; path: string; isDir: boolean }[]> => [
    { name: "src", path: "/home/user/project/src", isDir: true },
    { name: "README.md", path: "/home/user/project/README.md", isDir: false },
  ],
  add_allowed_path: async (): Promise<void> => undefined,
};

// Note: mock state (interrupted/errorArmed/pinned/nextPinId) resets naturally — each Playwright test loads a fresh page, re-evaluating this module.
