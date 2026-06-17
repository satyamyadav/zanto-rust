import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { platform as osPlatform } from "@tauri-apps/plugin-os";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

// OS platform string (e.g. "macos", "windows", "linux") so the UI can show the
// right shortcut glyphs (⌘ vs Ctrl).
export const platform = (): string => osPlatform();
export const isMac = (): boolean => osPlatform() === "macos";

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

// A context source (input): a file/dir path fed to every turn, with an enable
// toggle so it can be silenced without deleting.
export type ContextSource = { path: string; enabled: boolean };

export type Config = {
  model: string;
  endpoint: string;
  allowed_paths: string[];
  project_dir: string | null;
  context_sources: ContextSource[];
  selected_skill: string | null;
  max_context_turns: number | null;
  providers: ProviderDto[];
  active_provider: string | null;
};

// A discoverable markdown skill: file stem + a short body preview.
export type SkillDto = { name: string; preview: string };

export type ConfigPatch = Partial<Pick<Config, "model" | "endpoint" | "max_context_turns">> & {
  providers?: ProviderPatch[];
  active_provider?: string;
};

// A persisted display segment for a past assistant turn, mirroring `ChatSegment`
// (the runtime store type). Tool-call `args` is opaque JSON; `block` is a ChatBlock.
export type PersistedSegment =
  | { kind: "text"; text: string }
  | { kind: "reasoning"; text: string }
  | { kind: "tool_call"; id: string; name: string; args: any; output?: string; ok?: boolean }
  | { kind: "block"; block: ChatBlock };

// `blocks` carries persisted component blocks for a past assistant message
// (D1: `{ blocks: ChatBlock[] }`), restored as block segments on reopen.
// `segments` carries the full ordered display-segment list of an assistant turn
// (reasoning/tool_call/block/text) so it restores exactly as it rendered live;
// `stopped` marks an interrupted turn. Both are absent for legacy sessions, where
// the reopen path falls back to text + `blocks`.
export type RenderMsg = {
  role: "user" | "assistant";
  text: string;
  blocks?: { blocks: ChatBlock[] } | null;
  segments?: PersistedSegment[] | null;
  stopped?: boolean | null;
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
  // Storage class: "view" (ephemeral, render-only, pinnable) or "file" (durable
  // document). Drives the A-5 user Pin button (shown for "view" artifacts only).
  storage: string;
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

// A pinned view+data artifact (4b): a catalogue view persisted to the DB so it
// can be reopened. The browser re-renders it as
// `{ kind: "component", component_id, data, target }`.
export type PinnedArtifact = {
  id: number;
  component_id: string;
  title: string | null;
  target: Target;
  created_at: number;
  data: any;
};

// Thin typed wrappers over the Tauri IPC surface (commands + events).
export const ipc = {
  sendMessage: (text: string, imagePaths: string[] = []) =>
    invoke<ChatTurn>("send_message", { text, imagePaths }),
  interruptTurn: () => invoke<void>("interrupt_turn"),
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
  listSessionsPage: (offset: number, limit: number) =>
    invoke<SessionMeta[]>("list_sessions_page", { offset, limit }),
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
  // Save a copy of a stored document via a native save dialog. Resolves `true`
  // when a file was written, `false` if the user cancelled.
  saveArtifactCopy: (id: string) => invoke<boolean>("save_artifact_copy", { id }),
  // Reveal a stored document's file in the OS file manager.
  revealArtifact: (id: string) => invoke<void>("reveal_artifact", { id }),

  // Pinned view+data artifacts (4b): persisted catalogue views, reopenable.
  listPinnedArtifacts: () => invoke<PinnedArtifact[]>("list_pinned_artifacts"),
  readPinnedArtifact: (id: number) =>
    invoke<PinnedArtifact>("read_pinned_artifact", { id }),
  // Pin a rendered view+data artifact from the UI (A-5). Returns the record id.
  pinArtifact: (componentId: string, data: any, title?: string) =>
    invoke<number>("pin_artifact_cmd", { componentId, data, title: title ?? null }),

  // Config
  getConfig: () => invoke<Config>("get_config"),
  setConfig: (patch: ConfigPatch) => invoke<void>("set_config", { patch }),
  pickFolder: () => invoke<string | null>("pick_folder"),
  // Multi-select open-file dialog via the (already-registered) dialog plugin.
  // Returns the chosen absolute paths, or [] if cancelled.
  pickFiles: async (): Promise<string[]> => {
    const res = await invoke<string[] | string | null>("plugin:dialog|open", {
      options: { multiple: true, directory: false },
    });
    if (res == null) return [];
    return Array.isArray(res) ? res : [res];
  },
  browseDir: (path?: string) => invoke<FileEntry[]>("browse_dir", { path: path ?? null }),
  addAllowedPath: (path: string) => invoke<void>("add_allowed_path", { path }),
  addContextSource: (path: string) => invoke<void>("add_context_source", { path }),
  removeContextSource: (path: string) => invoke<void>("remove_context_source", { path }),
  toggleContextSource: (path: string, enabled: boolean) =>
    invoke<void>("toggle_context_source", { path, enabled }),
  setProjectDir: (path: string) => invoke<void>("set_project_dir", { path }),
  setApiKey: (provider: string, key: string) => invoke<void>("set_api_key", { provider, key }),
  clearApiKey: (provider: string) => invoke<void>("clear_api_key", { provider }),

  // Skills (user-selected markdown preprompts)
  listSkills: () => invoke<SkillDto[]>("list_skills"),
  setActiveSkill: (name: string | null) => invoke<void>("set_active_skill", { name }),

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
  // Emitted before `chat_done` when a turn was interrupted (Stop).
  onChatStopped: (cb: () => void): Promise<UnlistenFn> =>
    listen<null>("chat_stopped", () => cb()),

  // Native file drag-and-drop onto the window. Fires `enter`/`leave` (for the
  // dragover visual state) and `drop` (with the dropped absolute paths).
  onFileDrop: (handlers: {
    onEnter?: () => void;
    onLeave?: () => void;
    onDrop?: (paths: string[]) => void;
  }): Promise<UnlistenFn> =>
    getCurrentWebviewWindow().onDragDropEvent((e) => {
      const t = e.payload.type;
      if (t === "enter" || t === "over") handlers.onEnter?.();
      else if (t === "leave") handlers.onLeave?.();
      else if (t === "drop") {
        handlers.onLeave?.();
        handlers.onDrop?.(e.payload.paths);
      }
    }),
};
