<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Select from "$lib/components/ui/select";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { toast } from "svelte-sonner";
  import { mode, setMode } from "mode-watcher";
  import { density, setDensity, type Density } from "$lib/stores/theme.svelte";
  import { appStore, refreshConfig } from "$lib/stores/app.svelte";
  import { ipc, type ProviderPatch, type SkillDto } from "$lib/ipc";
  import EyeIcon from "@lucide/svelte/icons/eye";
  import EyeOffIcon from "@lucide/svelte/icons/eye-off";
  import FolderPlusIcon from "@lucide/svelte/icons/folder-plus";
  import XIcon from "@lucide/svelte/icons/x";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const NO_SKILL = "__none__";
  let skills = $state<SkillDto[]>([]);
  let activeSkill = $state(NO_SKILL);

  let activeProvider = $state("");
  let providers = $state<ProviderPatch[]>([]);
  let keyInput = $state("");
  let showKey = $state(false);
  let confirmClear = $state(false);
  // Tracks which provider the key field currently belongs to, so switching
  // providers clears a half-typed key / revealed key / open confirm banner.
  let keyForProvider = $state("");

  $effect(() => {
    if (open && appStore.config) {
      // Fall back to the first provider in the list so the UI is never blank.
      activeProvider = appStore.config.active_provider ?? appStore.config.providers[0]?.provider ?? "";
      providers = appStore.config.providers.map((p) => ({
        provider: p.provider,
        model: p.model,
        endpoint: p.endpoint,
      }));
      resetKeyState();
      activeSkill = appStore.config.selected_skill ?? NO_SKILL;
      loadSkills();
    }
  });

  async function loadSkills() {
    try {
      skills = await ipc.listSkills();
    } catch (e) {
      skills = [];
      toast.error("Could not load skills", { description: `${e}` });
    }
  }

  // Never carry one provider's key field, revealed state, or confirm banner
  // into another provider.
  $effect(() => {
    if (activeProvider !== keyForProvider) resetKeyState();
  });

  function resetKeyState() {
    keyInput = "";
    showKey = false;
    confirmClear = false;
    keyForProvider = activeProvider;
  }

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
      toast.error("Could not save settings", { description: `${e}` });
    }
  }

  async function saveKey() {
    if (!keyInput.trim() || !activeProvider) return;
    try {
      await ipc.setApiKey(activeProvider, keyInput.trim());
      keyInput = "";
      showKey = false;
      await refreshConfig();
      toast.success("API key saved");
    } catch (e) {
      toast.error("Could not save the API key", { description: `${e}` });
    }
  }

  async function clearKey() {
    if (!activeProvider) return;
    try {
      await ipc.clearApiKey(activeProvider);
      confirmClear = false;
      await refreshConfig();
      toast.success("API key cleared");
    } catch (e) {
      toast.error("Could not clear the API key", { description: `${e}` });
    }
  }

  async function pickFolder() {
    try {
      const f = await ipc.pickFolder();
      if (!f) return;
      await ipc.addAllowedPath(f);
      await refreshConfig();
      toast.success("Folder access granted", { description: f });
    } catch (e) {
      toast.error("Could not add the folder", { description: `${e}` });
    }
  }

  async function addContextSource() {
    try {
      const f = await ipc.pickFolder();
      if (!f) return;
      await ipc.addContextSource(f);
      await refreshConfig();
      toast.success("Context source added", { description: f });
    } catch (e) {
      toast.error("Could not add the context source", { description: `${e}` });
    }
  }

  async function removeContextSource(path: string) {
    try {
      await ipc.removeContextSource(path);
      await refreshConfig();
      toast.success("Context source removed", { description: path });
    } catch (e) {
      toast.error("Could not remove the context source", { description: `${e}` });
    }
  }

  async function selectSkill(name: string) {
    activeSkill = name;
    try {
      await ipc.setActiveSkill(name === NO_SKILL ? null : name);
    } catch (e) {
      toast.error("Could not set the active skill", { description: `${e}` });
    }
  }

  const densities: Density[] = ["compact", "normal", "relaxed"];
  const densityLabels: Record<Density, string> = {
    compact: "Compact",
    normal: "Normal",
    relaxed: "Relaxed",
  };
  const providerLabels: Record<string, string> = {
    anthropic: "Anthropic",
    openai: "OpenAI",
    gemini: "Gemini",
    ollama: "Ollama",
  };

  const activeProviderLabel = $derived(providerLabels[activeProvider] ?? activeProvider);
  const allowedPaths = $derived(appStore.config?.allowed_paths ?? []);
  const contextSources = $derived(appStore.config?.context_sources ?? []);
  const activeSkillLabel = $derived(
    activeSkill === NO_SKILL ? "None" : activeSkill
  );
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-lg">
    <Dialog.Header>
      <Dialog.Title class="font-display">Settings</Dialog.Title>
    </Dialog.Header>

    <div class="space-y-6 py-1">

      <!-- Provider & model -->
      <section class="space-y-3">
        <h3 class="font-display text-sm font-semibold tracking-tight">Provider &amp; model</h3>

        <div class="space-y-1.5">
          <span class="text-xs text-muted-foreground" id="cfg-provider-label">Active provider</span>
          <Select.Root type="single" bind:value={activeProvider}>
            <Select.Trigger
              class="w-full focus-visible:ring-2 focus-visible:ring-ring"
              aria-labelledby="cfg-provider-label"
            >
              {activeProviderLabel || "Choose a provider"}
            </Select.Trigger>
            <Select.Content>
              {#each (appStore.config?.providers ?? []) as p (p.provider)}
                <Select.Item value={p.provider} label={providerLabels[p.provider] ?? p.provider} />
              {/each}
            </Select.Content>
          </Select.Root>
        </div>

        {#if activeProvider}
          <div class="space-y-1.5">
            <label class="text-xs text-muted-foreground" for="cfg-prov-model">Model</label>
            <Input
              id="cfg-prov-model"
              class="font-mono focus-visible:ring-2 focus-visible:ring-ring"
              value={activeProviderPatch()?.model ?? ""}
              oninput={(e) => setActiveModel((e.target as HTMLInputElement).value)}
              placeholder="model name"
            />
          </div>

          {#if activeProvider === "ollama"}
            <div class="space-y-1.5">
              <label class="text-xs text-muted-foreground" for="cfg-prov-endpoint">Endpoint</label>
              <Input
                id="cfg-prov-endpoint"
                class="font-mono focus-visible:ring-2 focus-visible:ring-ring"
                value={activeProviderPatch()?.endpoint ?? ""}
                oninput={(e) => setActiveEndpoint((e.target as HTMLInputElement).value)}
                placeholder="http://localhost:11434/"
              />
            </div>
          {:else}
            <div class="space-y-1.5">
              <label class="flex items-center gap-1.5 text-xs text-muted-foreground" for="cfg-api-key">
                API key
                {#if activeProviderDto()?.has_key}
                  <span class="text-success">Saved</span>
                {/if}
              </label>
              <div class="flex gap-2">
                <div class="relative flex-1">
                  <Input
                    id="cfg-api-key"
                    type={showKey ? "text" : "password"}
                    bind:value={keyInput}
                    placeholder={activeProviderDto()?.has_key ? "Replace the saved key…" : "Enter the API key…"}
                    class="font-mono pr-9 focus-visible:ring-2 focus-visible:ring-ring"
                  />
                  <button
                    type="button"
                    class="absolute inset-y-0 right-0 grid w-9 place-items-center text-muted-foreground hover:text-foreground rounded-r-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                    onclick={() => (showKey = !showKey)}
                    aria-label={showKey ? "Hide the API key" : "Show the API key"}
                  >
                    {#if showKey}
                      <EyeOffIcon class="size-4" />
                    {:else}
                      <EyeIcon class="size-4" />
                    {/if}
                  </button>
                </div>
                <Button size="sm" onclick={saveKey} disabled={!keyInput.trim()}>Save key</Button>
                {#if activeProviderDto()?.has_key}
                  <Button
                    size="sm"
                    variant="outline"
                    onclick={() => (confirmClear = true)}
                  >
                    Clear
                  </Button>
                {/if}
              </div>
              {#if confirmClear}
                <div class="flex items-center gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-2.5 py-2 text-xs">
                  <span class="text-foreground">Remove the saved {activeProviderLabel} key?</span>
                  <Button size="xs" variant="destructive" class="ml-auto" onclick={clearKey}>
                    Clear key
                  </Button>
                  <Button size="xs" variant="ghost" onclick={() => (confirmClear = false)}>
                    Keep it
                  </Button>
                </div>
              {/if}
            </div>
          {/if}
        {/if}

        <Button size="sm" onclick={saveProviders}>Save changes</Button>
      </section>

      <!-- Appearance -->
      <section class="space-y-3">
        <h3 class="font-display text-sm font-semibold tracking-tight">Appearance</h3>
        <div class="space-y-1.5">
          <span class="text-xs text-muted-foreground" id="cfg-theme-label">Theme</span>
          <div class="flex gap-2" role="group" aria-labelledby="cfg-theme-label">
            <Button variant={mode.current === "light" ? "default" : "outline"} size="sm" onclick={() => setMode("light")}>
              Light
            </Button>
            <Button variant={mode.current === "dark" ? "default" : "outline"} size="sm" onclick={() => setMode("dark")}>
              Dark
            </Button>
          </div>
        </div>
        <div class="space-y-1.5">
          <span class="text-xs text-muted-foreground" id="cfg-density-label">Density</span>
          <div
            class="inline-flex rounded-lg border border-border bg-muted p-0.5"
            role="group"
            aria-labelledby="cfg-density-label"
          >
            {#each densities as d (d)}
              <button
                type="button"
                aria-pressed={density.value === d}
                class="rounded-md px-3 py-1 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {density.value === d
                  ? 'bg-background text-foreground shadow-sm'
                  : 'text-muted-foreground hover:text-foreground'}"
                onclick={() => setDensity(d)}
              >
                {densityLabels[d]}
              </button>
            {/each}
          </div>
        </div>
      </section>

      <!-- Folder access -->
      <section class="space-y-3">
        <h3 class="font-display text-sm font-semibold tracking-tight">Folder access</h3>
        {#if allowedPaths.length === 0}
          <p class="text-xs text-muted-foreground">
            No folders yet. Add one to let the assistant read and write files there.
          </p>
        {:else}
          <ul class="space-y-1">
            {#each allowedPaths as path (path)}
              <li class="truncate rounded-md bg-muted px-2.5 py-1.5 font-mono text-xs text-foreground" title={path}>
                {path}
              </li>
            {/each}
          </ul>
        {/if}
        <Button size="sm" variant="outline" onclick={pickFolder}>
          <FolderPlusIcon class="size-3.5" />
          Add folder…
        </Button>
      </section>

      <!-- Context sources -->
      <section class="space-y-3">
        <h3 class="font-display text-sm font-semibold tracking-tight">Context sources</h3>
        {#if contextSources.length === 0}
          <p class="text-xs text-muted-foreground">
            No context sources yet. Add a folder of notes to inject into every turn.
          </p>
        {:else}
          <ul class="space-y-1">
            {#each contextSources as path (path)}
              <li class="flex items-center gap-2 rounded-md bg-muted px-2.5 py-1.5">
                <span class="flex-1 truncate font-mono text-xs text-foreground" title={path}>{path}</span>
                <button
                  type="button"
                  class="grid size-5 place-items-center rounded text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  onclick={() => removeContextSource(path)}
                  aria-label="Remove context source"
                >
                  <XIcon class="size-3.5" />
                </button>
              </li>
            {/each}
          </ul>
        {/if}
        <Button size="sm" variant="outline" onclick={addContextSource}>
          <FolderPlusIcon class="size-3.5" />
          Add source…
        </Button>
      </section>

      <!-- Skill -->
      <section class="space-y-3">
        <h3 class="font-display text-sm font-semibold tracking-tight">Skill</h3>
        <div class="space-y-1.5">
          <span class="text-xs text-muted-foreground" id="cfg-skill-label">Active skill</span>
          <Select.Root type="single" value={activeSkill} onValueChange={selectSkill}>
            <Select.Trigger
              class="w-full focus-visible:ring-2 focus-visible:ring-ring"
              aria-labelledby="cfg-skill-label"
            >
              {activeSkillLabel}
            </Select.Trigger>
            <Select.Content>
              <Select.Item value={NO_SKILL} label="None" />
              {#each skills as s (s.name)}
                <Select.Item value={s.name} label={s.name} />
              {/each}
            </Select.Content>
          </Select.Root>
          {#if skills.length === 0}
            <p class="text-xs text-muted-foreground">
              No skills found. Add markdown files under <code class="font-mono">.zanto/skills</code>.
            </p>
          {/if}
        </div>
      </section>
    </div>
  </Dialog.Content>
</Dialog.Root>
