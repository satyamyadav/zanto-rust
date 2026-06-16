// App store: available micro-apps, the active app, and runtime config.
import { ipc, type AppManifest, type Config } from "$lib/ipc";

export const appStore = $state({
  apps: [] as AppManifest[],
  activeId: null as string | null,
  config: null as Config | null,
  loaded: false,
});

export function activeApp(): AppManifest | null {
  return appStore.apps.find((a) => a.id === appStore.activeId) ?? null;
}

export async function loadApps() {
  appStore.apps = await ipc.listApps();
  appStore.config = await ipc.getConfig();
  appStore.loaded = true;
}

export async function mountApp(id: string) {
  await ipc.mountApp(id);
  appStore.activeId = id;
}

export async function refreshConfig() {
  appStore.config = await ipc.getConfig();
}
