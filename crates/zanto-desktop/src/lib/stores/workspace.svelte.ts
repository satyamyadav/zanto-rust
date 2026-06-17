// Shared open state for the Workspace dialog so both the Sidebar button and the
// Composer context chip can open the same surface.
export const workspaceStore = $state({ open: false });

export function openWorkspace() {
  workspaceStore.open = true;
}
