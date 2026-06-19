<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Select from "$lib/components/ui/select";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { toast } from "svelte-sonner";
  import { mode, setMode } from "mode-watcher";
  import { density, setDensity, type Density } from "$lib/stores/theme.svelte";
  import { appStore, refreshConfig } from "$lib/stores/app.svelte";
  import { ipc, type ProviderPatch, type SkillDto, type GenerationParams } from "$lib/ipc";
  import EyeIcon from "@lucide/svelte/icons/eye";
  import EyeOffIcon from "@lucide/svelte/icons/eye-off";
  import FolderPlusIcon from "@lucide/svelte/icons/folder-plus";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const NO_SKILL = "__none__";
  const NO_EFFORT = "__default__";
  let skills = $state<SkillDto[]>([]);
  let activeSkill = $state(NO_SKILL);

  // Turns kept verbatim before older ones are LLM-summarized into context.
  // 0 = off (default truncation, no summarization). Applies on the next turn.
  let contextTurns = $state(0);

  let activeProvider = $state("");
  let providers = $state<ProviderPatch[]>([]);
  let keyInput = $state("");
  let showKey = $state(false);
  let confirmClear = $state(false);
  // Tracks which provider the key field currently belongs to, so switching
  // providers clears a half-typed key / revealed key / open confirm banner.
  let keyForProvider = $state("");

  // Model combobox state
  let modelList = $state<string[]>([]);
  let modelsLoading = $state(false);
  let modelsError = $state("");

  // Generation params state
  let gen = $state<GenerationParams>({});

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
      contextTurns = appStore.config.max_context_turns ?? 0;
      gen = { ...(appStore.config.generation ?? {}) };
      loadSkills();
    }
  });

  async function saveContext() {
    try {
      await ipc.setConfig({ max_context_turns: Math.max(0, Math.floor(contextTurns || 0)) });
      await refreshConfig();
      toast.success(contextTurns > 0 ? `Summarizing beyond ${contextTurns} turns` : "Summarization off");
    } catch (e) {
      toast.error("Could not save context settings", { description: `${e}` });
    }
  }

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
    if (activeProvider !== keyForProvider) {
      resetKeyState();
      modelList = [];
      modelsError = "";
    }
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

  async function selectSkill(name: string) {
    activeSkill = name;
    try {
      await ipc.setActiveSkill(name === NO_SKILL ? null : name);
    } catch (e) {
      toast.error("Could not set the active skill", { description: `${e}` });
    }
  }

  async function refreshModels() {
    if (!activeProvider) return;
    modelsLoading = true;
    modelsError = "";
    try {
      modelList = await ipc.listModels(activeProvider);
    } catch (e) {
      modelList = [];
      modelsError = "Couldn't load models — type the name manually.";
    } finally {
      modelsLoading = false;
    }
  }

  async function saveGeneration() {
    try {
      const clean = Object.fromEntries(
        Object.entries(gen).filter(([, v]) => v !== "" && v != null)
      ) as GenerationParams;
      await ipc.setConfig({ generation: clean });
      await refreshConfig();
      toast.success("Generation settings saved");
    } catch (e) {
      toast.error("Could not save generation settings", { description: `${e}` });
    }
  }

  // Step 1: Drive provider select from registry
  const registry = $derived(appStore.config?.provider_registry ?? []);
  function providerLabel(id: string): string {
    return registry.find((r) => r.id === id)?.label ?? id;
  }
  const activeProviderLabel = $derived(providerLabel(activeProvider));

  // Step 2: Capability checks instead of literal "ollama" string
  const activeInfo = $derived(registry.find((r) => r.id === activeProvider) ?? null);

  const densities: Density[] = ["compact", "normal", "relaxed"];
  const densityLabels: Record<Density, string> = {
    compact: "Compact",
    normal: "Normal",
    relaxed: "Relaxed",
  };

  const allowedPaths = $derived(appStore.config?.allowed_paths ?? []);
  const activeSkillLabel = $derived(
    activeSkill === NO_SKILL ? "None" : activeSkill
  );

  // Maps between the sentinel and undefined for the reasoning-effort select.
  const activeReasoningEffort = $derived(gen.reasoning_effort ?? NO_EFFORT);
  const activeReasoningEffortLabel = $derived(
    activeReasoningEffort === NO_EFFORT ? "Default" : activeReasoningEffort
  );
  function selectReasoningEffort(val: string) {
    gen.reasoning_effort = val === NO_EFFORT ? undefined : val;
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-[80vw]">
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
              {#each registry as r (r.id)}
                <Select.Item value={r.id} label={r.label} />
              {/each}
            </Select.Content>
          </Select.Root>
        </div>

        {#if activeProvider}
          <div class="space-y-1.5">
            <label class="text-xs text-muted-foreground" for="cfg-prov-model">Model</label>
            <div class="flex gap-2 items-center">
              <Input
                id="cfg-prov-model"
                class="font-mono flex-1 focus-visible:ring-2 focus-visible:ring-ring"
                list="cfg-model-options"
                value={activeProviderPatch()?.model ?? ""}
                oninput={(e) => setActiveModel((e.target as HTMLInputElement).value)}
                placeholder="model name"
              />
              <Button size="sm" variant="outline" onclick={refreshModels} disabled={modelsLoading}>
                {modelsLoading ? "Loading…" : "Refresh"}
              </Button>
            </div>
            <datalist id="cfg-model-options">
              {#each modelList as m (m)}<option value={m}></option>{/each}
            </datalist>
            {#if modelsError}
              <p class="text-xs text-muted-foreground">{modelsError}</p>
            {/if}
          </div>

          {#if activeInfo && !activeInfo.needs_key}
            <div class="space-y-1.5">
              <label class="text-xs text-muted-foreground" for="cfg-prov-endpoint">Endpoint</label>
              <Input
                id="cfg-prov-endpoint"
                class="font-mono focus-visible:ring-2 focus-visible:ring-ring"
                value={activeProviderPatch()?.endpoint ?? ""}
                oninput={(e) => setActiveEndpoint((e.target as HTMLInputElement).value)}
                placeholder={activeInfo?.default_endpoint ?? "http://localhost:11434/"}
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

      <!-- Context -->
      <section class="space-y-3">
        <h3 class="font-display text-sm font-semibold tracking-tight">Context</h3>
        <div class="space-y-1.5">
          <label class="text-xs text-muted-foreground" for="cfg-context-turns">
            Summarize beyond (turns)
          </label>
          <div class="flex items-center gap-2">
            <Input
              id="cfg-context-turns"
              type="number"
              min="0"
              step="1"
              bind:value={contextTurns}
              class="w-28 font-mono focus-visible:ring-2 focus-visible:ring-ring"
            />
            <Button size="sm" onclick={saveContext}>Save</Button>
          </div>
          <p class="text-xs text-muted-foreground">
            Keep the last N turns verbatim and LLM-summarize older ones into context.
            <span class="font-medium">0 = off</span> (default: keep the last 20, no summary). Applies on your next message.
          </p>
        </div>
      </section>

      <!-- Generation -->
      <section class="space-y-3">
        <h3 class="font-display text-sm font-semibold tracking-tight">Generation</h3>
        <div class="grid grid-cols-2 gap-3">
          <label class="space-y-1.5 text-xs text-muted-foreground">Temperature
            <Input type="number" step="0.1" min="0" bind:value={gen.temperature} class="font-mono" />
          </label>
          <label class="space-y-1.5 text-xs text-muted-foreground">Max tokens
            <Input type="number" step="1" min="1" bind:value={gen.max_tokens} class="font-mono" />
          </label>
          <label class="space-y-1.5 text-xs text-muted-foreground">Top-p
            <Input type="number" step="0.05" min="0" max="1" bind:value={gen.top_p} class="font-mono" />
          </label>
          <label class="space-y-1.5 text-xs text-muted-foreground">Seed
            <Input type="number" step="1" bind:value={gen.seed} class="font-mono" />
          </label>
        </div>
        <div class="space-y-1.5">
          <span class="text-xs text-muted-foreground" id="cfg-reasoning-label">Reasoning effort</span>
          <Select.Root type="single" value={activeReasoningEffort} onValueChange={selectReasoningEffort}>
            <Select.Trigger class="w-full" aria-labelledby="cfg-reasoning-label">
              {activeReasoningEffortLabel}
            </Select.Trigger>
            <Select.Content>
              <Select.Item value={NO_EFFORT} label="Default" />
              {#each ["none", "minimal", "low", "medium", "high", "xhigh"] as e (e)}
                <Select.Item value={e} label={e} />
              {/each}
            </Select.Content>
          </Select.Root>
        </div>
        <Button size="sm" onclick={saveGeneration}>Save generation</Button>
        <p class="text-xs text-muted-foreground">
          Empty fields use the provider default. Unsupported options are ignored per provider.
        </p>
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
