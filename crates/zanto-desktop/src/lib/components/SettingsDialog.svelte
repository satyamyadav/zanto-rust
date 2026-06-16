<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { toast } from "svelte-sonner";
  import { mode, setMode } from "mode-watcher";
  import { density, setDensity, type Density } from "$lib/stores/theme.svelte";
  import { appStore, refreshConfig } from "$lib/stores/app.svelte";
  import { ipc, type ProviderPatch } from "$lib/ipc";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  let activeProvider = $state("");
  let providers = $state<ProviderPatch[]>([]);
  let keyInput = $state("");

  $effect(() => {
    if (open && appStore.config) {
      // Fall back to the first provider in the list so the UI is never blank.
      activeProvider = appStore.config.active_provider ?? appStore.config.providers[0]?.provider ?? "";
      providers = appStore.config.providers.map((p) => ({
        provider: p.provider,
        model: p.model,
        endpoint: p.endpoint,
      }));
      keyInput = "";
    }
  });

  function activeProviderDto() {
    return appStore.config?.providers.find((p) => p.provider === activeProvider) ?? null;
  }

  function activeProviderPatch(): ProviderPatch | undefined {
    return providers.find((p) => p.provider === activeProvider);
  }

  function setActiveModel(val: string) {
    providers = providers.map((p) =>
      p.provider === activeProvider ? { ...p, model: val } : p
    );
  }

  function setActiveEndpoint(val: string) {
    providers = providers.map((p) =>
      p.provider === activeProvider ? { ...p, endpoint: val || null } : p
    );
  }

  async function saveProviders() {
    try {
      await ipc.setConfig({ providers, active_provider: activeProvider || undefined });
      await refreshConfig();
      toast.success("Settings saved");
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  async function saveKey() {
    if (!keyInput.trim() || !activeProvider) return;
    try {
      await ipc.setApiKey(activeProvider, keyInput.trim());
      keyInput = "";
      await refreshConfig();
      toast.success("API key saved");
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  async function clearKey() {
    if (!activeProvider) return;
    try {
      await ipc.clearApiKey(activeProvider);
      await refreshConfig();
      toast.success("API key cleared");
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  async function pickFolder() {
    try {
      const f = await ipc.pickFolder();
      if (!f) return;
      await ipc.addAllowedPath(f);
      await refreshConfig();
      toast.success(`Folder access granted`, { description: f });
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  const densities: Density[] = ["compact", "normal", "relaxed"];
  const providerLabels: Record<string, string> = {
    anthropic: "Anthropic",
    openai: "OpenAI",
    gemini: "Gemini",
    ollama: "Ollama",
  };
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-lg">
    <Dialog.Header>
      <Dialog.Title>Settings</Dialog.Title>
    </Dialog.Header>

    <div class="space-y-6 py-1">

      <!-- Provider & model -->
      <section class="space-y-2">
        <h3 class="text-sm font-medium">Provider &amp; model</h3>

        <div class="space-y-1">
          <label class="text-xs text-muted-foreground" for="cfg-provider">Active provider</label>
          <select
            id="cfg-provider"
            class="w-full rounded-md border border-input bg-background px-3 py-1.5 text-sm"
            bind:value={activeProvider}
          >
            {#each (appStore.config?.providers ?? []) as p}
              <option value={p.provider}>{providerLabels[p.provider] ?? p.provider}</option>
            {/each}
          </select>
        </div>

        {#if activeProvider}
          <div class="space-y-1">
            <label class="text-xs text-muted-foreground" for="cfg-prov-model">Model</label>
            <Input
              id="cfg-prov-model"
              value={activeProviderPatch()?.model ?? ""}
              oninput={(e) => setActiveModel((e.target as HTMLInputElement).value)}
              placeholder="model name"
            />
          </div>

          {#if activeProvider === "ollama"}
            <div class="space-y-1">
              <label class="text-xs text-muted-foreground" for="cfg-prov-endpoint">Endpoint</label>
              <Input
                id="cfg-prov-endpoint"
                value={activeProviderPatch()?.endpoint ?? ""}
                oninput={(e) => setActiveEndpoint((e.target as HTMLInputElement).value)}
                placeholder="http://localhost:11434/"
              />
            </div>
          {/if}

          {#if activeProvider !== "ollama"}
            <div class="space-y-1">
              <label class="text-xs text-muted-foreground" for="cfg-api-key">
                API key
                {#if activeProviderDto()?.has_key}
                  <span class="ml-1 text-green-600 dark:text-green-400">Saved ✓</span>
                {/if}
              </label>
              <div class="flex gap-2">
                <Input
                  id="cfg-api-key"
                  type="password"
                  bind:value={keyInput}
                  placeholder={activeProviderDto()?.has_key ? "replace saved key…" : "enter API key…"}
                  class="flex-1"
                />
                <Button size="sm" onclick={saveKey} disabled={!keyInput.trim()}>Save key</Button>
                {#if activeProviderDto()?.has_key}
                  <Button size="sm" variant="outline" onclick={clearKey}>Clear</Button>
                {/if}
              </div>
            </div>
          {/if}
        {/if}

        <Button size="sm" onclick={saveProviders}>Save</Button>
      </section>

      <!-- Appearance -->
      <section class="space-y-2">
        <h3 class="text-sm font-medium">Appearance</h3>
        <div class="space-y-1">
          <span class="text-xs text-muted-foreground">Theme</span>
          <div class="flex gap-2">
            <Button variant={mode.current === "light" ? "default" : "outline"} size="sm" onclick={() => setMode("light")}>
              Light
            </Button>
            <Button variant={mode.current === "dark" ? "default" : "outline"} size="sm" onclick={() => setMode("dark")}>
              Dark
            </Button>
          </div>
        </div>
        <div class="space-y-1">
          <span class="text-xs text-muted-foreground">Density</span>
          <div class="flex gap-2">
            {#each densities as d}
              <Button variant={density.value === d ? "default" : "outline"} size="sm" onclick={() => setDensity(d)}>
                {d}
              </Button>
            {/each}
          </div>
        </div>
      </section>

      <!-- Folder access -->
      <section class="space-y-2">
        <h3 class="text-sm font-medium">Folder access</h3>
        <div class="text-xs text-muted-foreground break-all">
          {appStore.config?.allowed_paths?.join(", ") || "none"}
        </div>
        <Button size="sm" variant="outline" onclick={pickFolder}>Add folder…</Button>
      </section>
    </div>
  </Dialog.Content>
</Dialog.Root>
