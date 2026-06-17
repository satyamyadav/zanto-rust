import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Target = "inline" | "canvas";

export type ChatBlock =
  | { kind: "markdown"; text: string }
  | { kind: "component"; component_id: string; data: any; target: Target };

export type ChatTurn = { blocks: ChatBlock[] };

export type ComponentDecl = { id: string; schema: any };
export type StartAction = { label: string; prompt: string };
export type AppManifest = {
  id: string;
  name: string;
  description: string;
  stores: string[];
  components: ComponentDecl[];
  start_actions: StartAction[];
};

export type InteractionField = {
  name: string;
  label: string;
  type: "text" | "select" | "confirm";
  options?: string[];
};
export type InteractionStep = { fields: InteractionField[] };
export type InteractionRequest = {
  id: string;
  kind: "approval" | "form";
  // approval
  op?: string;
  path?: string;
  resolved?: string;
  // form
  title?: string;
  steps?: InteractionStep[];
};

export type SessionMeta = {
  id: string;
  title: string;
  workspace: string;
  app_id: string | null;
  created_at: number;
  updated_at: number;
  message_count: number;
  archived: boolean;
};

export type ProviderDto = {
  provider: string;
  model: string;
  endpoint: string | null;
  has_key: boolean;
};

export type ProviderPatch = {
  provider: string;
  model: string;
  endpoint: string | null;
};

export type Config = {
  model: string;
  endpoint: string;
  allowed_paths: string[];
  max_context_turns: number | null;
  providers: ProviderDto[];
  active_provider: string | null;
};

export type ConfigPatch = Partial<Pick<Config, "model" | "endpoint" | "max_context_turns">> & {
  providers?: ProviderPatch[];
  active_provider?: string;
};

// `blocks` carries persisted component blocks for a past assistant message
// (D1: `{ blocks: ChatBlock[] }`), restored as block segments on reopen.
export type RenderMsg = {
  role: "user" | "assistant";
  text: string;
  blocks?: { blocks: ChatBlock[] } | null;
};

// A filesystem entry from `browse_dir` (B1). `path = undefined` lists the
// allowed roots; passing a dir's `path` descends into it.
export type FileEntry = { name: string; path: string; isDir: boolean };

export type ToolCallEvent = { id: string; name: string; args: any };
export type ToolResultEvent = { id: string; output: string; ok: boolean };

export type ArtifactDef = {
  id: string;
  description: string;
  when_to_use: string;
  data_schema: any;
};

export type ArtifactKind = "text" | "markdown" | "image" | "json";
export type ArtifactScope = "project" | "global";

// A stored artifact's manifest entry (E4 browser list).
export type StoredArtifactRef = {
  id: string;
  kind: ArtifactKind;
  title: string;
  rel_path: string;
  scope: ArtifactScope;
  created_at: number;
};

// A stored artifact with its content. Text/markdown/json carry UTF-8 `content`;
// images carry base64 `content` with `is_image` and a `mime` hint.
export type StoredArtifact = StoredArtifactRef & {
  is_image: boolean;
  mime?: string;
  content: string;
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
  loadSessionPage: (id: string, offset: number, limit: number) =>
    invoke<RenderMsg[]>("load_session_page", { id, offset, limit }),
  newSession: () => invoke<string>("new_session"),
  deleteSession: (id: string) => invoke<void>("delete_session", { id }),
  renameSession: (id: string, title: string) => invoke<void>("rename_session", { id, title }),
  archiveSession: (id: string) => invoke<void>("archive_session", { id }),
  unarchiveSession: (id: string) => invoke<void>("unarchive_session", { id }),
  listArchivedSessions: () => invoke<SessionMeta[]>("list_archived_sessions"),

  // Stored artifacts (E4 browser)
  listStoredArtifacts: (scope?: ArtifactScope) =>
    invoke<StoredArtifactRef[]>("list_stored_artifacts_cmd", { scope: scope ?? null }),
  readStoredArtifact: (id: string) =>
    invoke<StoredArtifact>("read_stored_artifact_cmd", { id }),

  // Config
  getConfig: () => invoke<Config>("get_config"),
  setConfig: (patch: ConfigPatch) => invoke<void>("set_config", { patch }),
  pickFolder: () => invoke<string | null>("pick_folder"),
  browseDir: (path?: string) => invoke<FileEntry[]>("browse_dir", { path: path ?? null }),
  addAllowedPath: (path: string) => invoke<void>("add_allowed_path", { path }),
  setApiKey: (provider: string, key: string) => invoke<void>("set_api_key", { provider, key }),
  clearApiKey: (provider: string) => invoke<void>("clear_api_key", { provider }),

  // HITL interaction channel (approvals + agent forms)
  respond: (requestId: string, value: unknown) => invoke<void>("respond", { requestId, value }),
  onInteractionRequest: (cb: (r: InteractionRequest) => void): Promise<UnlistenFn> =>
    listen<InteractionRequest>("interaction_request", (e) => cb(e.payload)),

  // Streaming turn events: text deltas, reasoning deltas, tool calls/results,
  // component blocks, then a final `done`.
  onChatChunk: (cb: (text: string) => void): Promise<UnlistenFn> =>
    listen<{ text: string }>("chat_chunk", (e) => cb(e.payload.text)),
  onChatReasoning: (cb: (text: string) => void): Promise<UnlistenFn> =>
    listen<{ text: string }>("chat_reasoning", (e) => cb(e.payload.text)),
  onChatToolCall: (cb: (call: ToolCallEvent) => void): Promise<UnlistenFn> =>
    listen<ToolCallEvent>("chat_tool_call", (e) => cb(e.payload)),
  onChatToolResult: (cb: (result: ToolResultEvent) => void): Promise<UnlistenFn> =>
    listen<ToolResultEvent>("chat_tool_result", (e) => cb(e.payload)),
  onChatBlock: (cb: (block: ChatBlock) => void): Promise<UnlistenFn> =>
    listen<{ block: ChatBlock }>("chat_block", (e) => cb(e.payload.block)),
  onChatDone: (cb: () => void): Promise<UnlistenFn> =>
    listen<null>("chat_done", () => cb()),
};
