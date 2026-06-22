import type {
  Config, AppManifest, ArtifactDef, SessionMeta, ChatTurn,
} from "$lib/ipc";
import { emit } from "./event";

import getConfigFx from "../../../contract/fixtures/get_config.json";
import listAppsFx from "../../../contract/fixtures/list_apps.json";
import getCatalogueFx from "../../../contract/fixtures/get_catalogue.json";
import listSessionsFx from "../../../contract/fixtures/list_sessions.json";
import newSessionFx from "../../../contract/fixtures/new_session.json";

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
  interrupt_turn: async () => undefined,
};

export function resetBackend(): void {
  // re-seed mutable state here as commands with side effects are added.
}

// Silence unused-import lint until streaming handlers (Task 3) use it.
void emit;
void (null as unknown as ChatTurn);
