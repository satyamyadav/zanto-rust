import type {
  Config, AppManifest, ArtifactDef, SessionMeta, ChatTurn,
} from "$lib/ipc";
import { emit } from "./event";
import { pickScenario } from "./scenarios";

import getConfigFx from "../../../contract/fixtures/get_config.json";
import listAppsFx from "../../../contract/fixtures/list_apps.json";
import getCatalogueFx from "../../../contract/fixtures/get_catalogue.json";
import listSessionsFx from "../../../contract/fixtures/list_sessions.json";
import newSessionFx from "../../../contract/fixtures/new_session.json";
import listPinnedFx from "../../../contract/fixtures/list_pinned_artifacts.json";
import loadSessionFx from "../../../contract/fixtures/load_session.json";

let interrupted = false;
let pinned: any[] = listPinnedFx.response.slice();
let nextPinId = pinned.length + 1;

// Each handler is keyed by the exact `invoke` command name used in ipc.ts.
// Typed return values turn the fixture JSON into a compile-time contract.
export const backend: Record<string, (args: any) => Promise<unknown>> = {
  get_config: async (): Promise<Config> => getConfigFx.response as Config,
  list_apps: async (): Promise<AppManifest[]> => listAppsFx.response,
  get_catalogue: async (): Promise<ArtifactDef[]> => getCatalogueFx.response,
  list_sessions: async (): Promise<SessionMeta[]> => listSessionsFx.response,
  list_sessions_page: async (): Promise<SessionMeta[]> => listSessionsFx.response,
  list_archived_sessions: async (): Promise<SessionMeta[]> => listSessionsFx.response,
  new_session: async (): Promise<string> => newSessionFx.response,
  mount_app: async () => undefined,
  unmount_app: async () => undefined,
  send_message: async (args: { text?: string }): Promise<ChatTurn> => {
    interrupted = false;
    const sc = pickScenario(args?.text ?? "");
    for (const ev of sc.events) {
      if (interrupted) break;
      emit(ev.event, ev.payload);
      await Promise.resolve(); // yield a microtask so the UI updates between deltas
    }
    if (interrupted) { emit("chat_stopped", null); emit("chat_done", null); }
    return sc.response;
  },
  interrupt_turn: async () => { interrupted = true; },
  load_session: async (): Promise<any> => loadSessionFx.response,
  load_session_page: async (): Promise<any> => loadSessionFx.response,
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
};

export function resetBackend(): void {
  interrupted = false;
  pinned = listPinnedFx.response.slice();
  nextPinId = pinned.length + 1;
  // re-seed mutable state here as commands with side effects are added.
}
