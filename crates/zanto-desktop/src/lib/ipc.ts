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

// Thin typed wrappers over the Tauri IPC surface (commands + events).
export const ipc = {
  sendMessage: (text: string) => invoke<ChatTurn>("send_message", { text }),
  listApps: () => invoke<AppManifest[]>("list_apps"),
  mountApp: (id: string) => invoke<void>("mount_app", { id }),
  unmountApp: () => invoke<void>("unmount_app"),
  queryApp: (id: string, query: string, args: any = {}) =>
    invoke<any>("query_app", { id, query, args }),
  runAppAction: (id: string, action: string, args: any = {}) =>
    invoke<any>("run_app_action", { id, action, args }),
  newSession: () => invoke<void>("new_session"),
  approve: (requestId: string, response: "once" | "session" | "forever" | "deny") =>
    invoke<void>("approve", { requestId, response }),
  onApprovalRequest: (cb: (r: ApprovalRequest) => void): Promise<UnlistenFn> =>
    listen<ApprovalRequest>("approval_request", (e) => cb(e.payload)),
};
