import type {
  Config, AppManifest, ArtifactDef, SessionMeta, ChatTurn, SkillDto, SkillScope,
} from "$lib/ipc";
import { emit } from "./event";
import { defaultScenario, pickScenario } from "./scenarios";
import { financeQuery, financeAction } from "./finance";

import getConfigFx from "../../../contract/fixtures/get_config.json";
import listAppsFx from "../../../contract/fixtures/list_apps.json";
import getCatalogueFx from "../../../contract/fixtures/get_catalogue.json";
import listSessionsFx from "../../../contract/fixtures/list_sessions.json";
import newSessionFx from "../../../contract/fixtures/new_session.json";
import listPinnedFx from "../../../contract/fixtures/list_pinned_artifacts.json";
import loadSessionFx from "../../../contract/fixtures/load_session.json";

let interrupted = false;
let activeSkill: string | null = null;

// In-memory skill store for the editor (CRUD). Keyed by "<scope>/<name>" so
// project and global skills of the same name coexist. Seeded with the two skills
// the picker tests expect (global scope).
type MockSkill = { name: string; body: string; scope: SkillScope };
const skillKey = (scope: SkillScope, name: string) => `${scope}/${name}`;
const mockSkills = new Map<string, MockSkill>([
  ["global/reviewer", { name: "reviewer", body: "Review code for bugs and clarity.", scope: "global" }],
  ["global/researcher", { name: "researcher", body: "Find and cite sources.", scope: "global" }],
  ["project/reviewer", { name: "reviewer", body: "PROJECT reviewer.", scope: "project" }],
]);
// Mirror the core name validation so the mock rejects unsafe names like the real
// backend does (keeps the editor's error paths exercisable in dev:mock).
function validateMockSkillName(name: string) {
  const n = name.trim();
  if (!n) throw new Error("Skill name cannot be empty");
  if (n.startsWith(".")) throw new Error("Skill name cannot start with a dot");
  if (n.includes("/") || n.includes("\\") || n.includes("..")) {
    throw new Error("Skill name cannot contain path separators");
  }
  if (!/^[\p{L}\p{N} _-]+$/u.test(n)) {
    throw new Error("Skill name may only contain letters, digits, spaces, - and _");
  }
}
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

// Session with a user message that has a persisted attachment (D6 reopen test).
const attachmentSession = [
  {
    role: "user",
    text: "Here is the attached file",
    blocks: null,
    segments: null,
    stopped: null,
    attachments: [{ path: "/home/user/docs/report.pdf", name: "report.pdf", is_image: false }],
  },
  {
    role: "assistant",
    text: "Got it.",
    blocks: null,
    segments: null,
    stopped: null,
  },
];

// Session with a user message that has a persisted IMAGE attachment (D7 image-viewer test).
const imageSession = [
  {
    role: "user",
    text: "Check this screenshot",
    blocks: null,
    segments: null,
    stopped: null,
    attachments: [{ path: "/home/user/pics/screenshot.png", name: "screenshot.png", is_image: true }],
  },
  {
    role: "assistant",
    text: "Looks good.",
    blocks: null,
    segments: null,
    stopped: null,
  },
];

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
    {
      id: "sess-attachments",
      title: "Attachment session",
      workspace: "/home/user/project",
      app_id: null,
      created_at: 1700000300,
      updated_at: 1700000400,
      message_count: 2,
      archived: false,
    } satisfies SessionMeta,
    {
      id: "sess-images",
      title: "Image session",
      workspace: "/home/user/project",
      app_id: null,
      created_at: 1700000500,
      updated_at: 1700000600,
      message_count: 2,
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
  load_session: async (a: { id?: string }): Promise<any> => {
    if (a?.id === "sess-long") return longSession;
    if (a?.id === "sess-attachments") return attachmentSession;
    if (a?.id === "sess-images") return imageSession;
    return loadSessionFx.response;
  },
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
  query_app: async (a: { id: string; query: string; args: any }): Promise<any> =>
    a?.id === "finance" ? financeQuery(a.query, a.args) : ({ income: 2000, spent: 12.5, net: 1987.5, by_category: { dining: 12.5 } }),
  run_app_action: async (a: { id: string; action: string; args: any }): Promise<any> =>
    a?.id === "finance" ? financeAction(a.action, a.args) : ({}),
  // Minimal seed so the @-tag autocomplete (C-8) has entries to display.
  // When descending into "src", returns a child file so keyboard descend is testable.
  browse_dir: async (a: { path?: string }): Promise<{ name: string; path: string; isDir: boolean }[]> => {
    if (a?.path === "/home/user/project/src") {
      return [{ name: "main.ts", path: "/home/user/project/src/main.ts", isDir: false }];
    }
    return [
      { name: "src", path: "/home/user/project/src", isDir: true },
      { name: "README.md", path: "/home/user/project/README.md", isDir: false },
    ];
  },
  // Return a tiny 1×1 transparent PNG as a data-URL (for image-viewer tests).
  read_image_data_url: async (_a: { path: string }): Promise<string> =>
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==",
  // Open a path with the OS default app — no-op in the mock.
  open_path: async (_a: { path: string }): Promise<void> => undefined,
  // Native file-picker dialog (used by ipc.pickFiles → `plugin:dialog|open`).
  // Returns a single test file path so attachment tests can trigger pick without
  // a real native dialog. The key must match the exact invoke command name.
  "plugin:dialog|open": async (): Promise<string[]> => ["/home/user/docs/notes.txt"],
  add_allowed_path: async (): Promise<void> => undefined,
  list_skills: async (): Promise<SkillDto[]> =>
    // Project scope first, then global — matching the real IPC's iteration order
    // so name-dedup downstream resolves project-shadows-global the same way.
    [...mockSkills.values()]
      .sort((a, b) => (a.scope === b.scope ? 0 : a.scope === "project" ? -1 : 1))
      .map((s) => ({
        name: s.name,
        preview: s.body.trim().slice(0, 120),
        scope: s.scope,
      })),
  set_active_skill: async (a: { name: string | null }): Promise<void> => { activeSkill = a?.name ?? null; },
  read_skill: async (a: { name: string; scope: SkillScope }): Promise<string> => {
    const s = mockSkills.get(skillKey(a.scope, a.name));
    if (!s) throw new Error(`Skill '${a.name}' not found in ${a.scope} scope`);
    return s.body;
  },
  save_skill: async (a: { name: string; scope: SkillScope; body: string; overwrite: boolean }): Promise<SkillDto> => {
    validateMockSkillName(a.name);
    const name = a.name.trim();
    const key = skillKey(a.scope, name);
    if (!a.overwrite && mockSkills.has(key)) throw new Error(`A skill named '${name}' already exists`);
    mockSkills.set(key, { name, body: a.body, scope: a.scope });
    return { name, preview: a.body.trim().slice(0, 120), scope: a.scope };
  },
  delete_skill: async (a: { name: string; scope: SkillScope }): Promise<void> => {
    const key = skillKey(a.scope, a.name.trim());
    if (!mockSkills.has(key)) throw new Error(`Skill '${a.name}' does not exist`);
    mockSkills.delete(key);
    if (activeSkill === a.name.trim()) activeSkill = null;
  },
  rename_skill: async (a: { old: string; new: string; scope: SkillScope }): Promise<void> => {
    validateMockSkillName(a.new);
    const fromKey = skillKey(a.scope, a.old.trim());
    const toKey = skillKey(a.scope, a.new.trim());
    const s = mockSkills.get(fromKey);
    if (!s) throw new Error(`Skill '${a.old}' does not exist`);
    if (mockSkills.has(toKey)) throw new Error(`A skill named '${a.new}' already exists`);
    mockSkills.delete(fromKey);
    mockSkills.set(toKey, { name: a.new.trim(), body: s.body, scope: a.scope });
    if (activeSkill === a.old.trim()) activeSkill = a.new.trim();
  },
  respond: async (_a: { requestId: string; value: unknown }): Promise<void> => {},
};

// Note: mock state (interrupted/errorArmed/pinned/nextPinId) resets naturally — each Playwright test loads a fresh page, re-evaluating this module.
