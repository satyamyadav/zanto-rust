import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Target = "inline" | "canvas";

export type ChatBlock =
  | { kind: "markdown"; text: string }
  | { kind: "component"; component_id: string; data: any; target: Target };

export type ChatTurn = { blocks: ChatBlock[] };

export type ComponentDecl = { id: string; schema: any };
export type AppManifest = {
  id: string;
  name: string;
  description: string;
  stores: string[];
  components: ComponentDecl[];
};

export type ApprovalRequest = { id: string; path: string; op: string; resolved: string };

export type SessionMeta = {
  id: string;
  title: string;
  workspace: string;
  app_id: string | null;
  created_at: number;
  updated_at: number;
  message_count: number;
};

export type Config = {
  model: string;
  endpoint: string;
  allowed_paths: string[];
  max_context_turns: number | null;
};

export type ConfigPatch = Partial<Pick<Config, "model" | "endpoint" | "max_context_turns">>;

export type RenderMsg = { role: "user" | "assistant"; text: string };

export type ArtifactDef = {
  id: string;
  description: string;
  when_to_use: string;
  data_schema: any;
};

// Thin typed wrappers over the Tauri IPC surface (commands + events).
export const ipc = {
  sendMessage: (text: string) => invoke<ChatTurn>("send_message", { text }),
  listApps: () => invoke<AppManifest[]>("list_apps"),
  getCatalogue: () => invoke<ArtifactDef[]>("get_catalogue"),
  mountApp: (id: string) => invoke<void>("mount_app", { id }),
  unmountApp: () => invoke<void>("unmount_app"),
  queryApp: (id: string, query: string, args: any = {}) =>
    invoke<any>("query_app", { id, query, args }),
  runAppAction: (id: string, action: string, args: any = {}) =>
    invoke<any>("run_app_action", { id, action, args }),
  // Sessions (scoped to the active app)
  listSessions: () => invoke<SessionMeta[]>("list_sessions"),
  loadSession: (id: string) => invoke<RenderMsg[]>("load_session", { id }),
  newSession: () => invoke<string>("new_session"),
  deleteSession: (id: string) => invoke<void>("delete_session", { id }),
  renameSession: (id: string, title: string) => invoke<void>("rename_session", { id, title }),

  // Config
  getConfig: () => invoke<Config>("get_config"),
  setConfig: (patch: ConfigPatch) => invoke<void>("set_config", { patch }),
  pickFolder: () => invoke<string | null>("pick_folder"),
  addAllowedPath: (path: string) => invoke<void>("add_allowed_path", { path }),

  approve: (requestId: string, response: "once" | "session" | "forever" | "deny") =>
    invoke<void>("approve", { requestId, response }),
  onApprovalRequest: (cb: (r: ApprovalRequest) => void): Promise<UnlistenFn> =>
    listen<ApprovalRequest>("approval_request", (e) => cb(e.payload)),
};
