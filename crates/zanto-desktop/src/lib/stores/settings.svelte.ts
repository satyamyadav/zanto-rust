// Shared open state for the Settings dialog so the Sidebar gear, the Composer
// context chip, and the ArtifactBrowser "Set project" button can all open the
// same surface — optionally deep-linked to a specific tab.
type SettingsSection =
  | "providers"
  | "project"
  | "context-sources"
  | "theme"
  | "folders";

export const settingsStore = $state<{ open: boolean; section: SettingsSection | undefined }>({
  open: false,
  section: undefined,
});

// Open Settings, optionally jumping straight to `section`.
export function openSettings(section?: SettingsSection) {
  settingsStore.section = section;
  settingsStore.open = true;
}
