// Shared open state for the Skills editor dialog, so the composer's /skill menu
// ("Manage skills…") can open the same surface. Mirrors settingsStore.
export const skillsStore = $state<{ open: boolean }>({ open: false });

export function openSkillsEditor() {
  skillsStore.open = true;
}
