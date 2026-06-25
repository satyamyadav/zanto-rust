// Artifact Hub: the right panel is a hub of open artifact tabs plus a Library
// (the list of all stored documents). Tabs come from two sources:
//   - "stored": an artifact persisted in the store, referenced by id (opened
//     from the Library, or after a chat document is Saved).
//   - "doc": an unsaved document the assistant generated, carried as raw text
//     until the user Saves it (which converts the tab to "stored").
// Pure client state — no backend change. The panel (ArtifactHub.svelte) renders
// it; chat messages and the Library push tabs into it.

export type HubTab =
  | { key: string; kind: "stored"; id: string; title: string }
  | { key: string; kind: "doc"; title: string; text: string }
  | { key: string; kind: "pinned"; id: number; componentId: string; data: unknown; title: string };

export const hubStore = $state({
  open: [] as HubTab[],
  activeKey: null as string | null,
});

let nextDocKey = 1;

// Open (or focus) a stored artifact tab by id. Idempotent: the same id reuses
// its existing tab instead of stacking duplicates.
export function openStored(id: string, title: string) {
  const key = `stored:${id}`;
  if (!hubStore.open.some((t) => t.key === key)) {
    hubStore.open = [...hubStore.open, { key, kind: "stored", id, title }];
  }
  hubStore.activeKey = key;
}

// Open a new unsaved-document tab from raw text. Each call is a distinct tab
// (no natural id to dedupe on). Returns the tab key so the caller can track it.
export function openDoc(title: string, text: string): string {
  const key = `doc:${nextDocKey++}`;
  hubStore.open = [...hubStore.open, { key, kind: "doc", title, text }];
  hubStore.activeKey = key;
  return key;
}

// Replace a "doc" tab with the "stored" identity it became after Save, keeping
// the same tab position/focus so the viewer doesn't jump.
export function markDocSaved(key: string, id: string, title: string) {
  const i = hubStore.open.findIndex((t) => t.key === key);
  if (i === -1) return;
  const newKey = `stored:${id}`;
  const next = [...hubStore.open];
  next[i] = { key: newKey, kind: "stored", id, title };
  hubStore.open = next;
  if (hubStore.activeKey === key) hubStore.activeKey = newKey;
}

// Close a tab; if it was active, focus the neighbour (next, else previous).
export function closeTab(key: string) {
  const i = hubStore.open.findIndex((t) => t.key === key);
  if (i === -1) return;
  const next = hubStore.open.filter((t) => t.key !== key);
  hubStore.open = next;
  if (hubStore.activeKey === key) {
    hubStore.activeKey = next[i]?.key ?? next[i - 1]?.key ?? null;
  }
}

export function focusTab(key: string) {
  hubStore.activeKey = key;
}

// Drop a stored tab by id (e.g. after it's deleted from the store).
export function removeStored(id: string) {
  closeTab(`stored:${id}`);
}

// Open (or focus) a pinned view tab (a saved chart/table re-rendered from its
// DB record). Idempotent by pinned id.
export function openPinned(id: number, componentId: string, data: unknown, title: string) {
  const key = `pinned:${id}`;
  if (!hubStore.open.some((t) => t.key === key)) {
    hubStore.open = [...hubStore.open, { key, kind: "pinned", id, componentId, data, title }];
  }
  hubStore.activeKey = key;
}
