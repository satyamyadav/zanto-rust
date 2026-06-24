<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import * as Select from "$lib/components/ui/select";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import GenerationFields from "$lib/components/GenerationFields.svelte";
  import { toast } from "svelte-sonner";
  import { mode, setMode } from "mode-watcher";
  import { density, setDensity, type Density } from "$lib/stores/theme.svelte";
  import { untrack } from "svelte";
  import { appStore, refreshConfig } from "$lib/stores/app.svelte";
  import { ipc, type ProviderPatch, type SkillDto, type GenerationParams } from "$lib/ipc";
  import EyeIcon from "@lucide/svelte/icons/eye";
  import EyeOffIcon from "@lucide/svelte/icons/eye-off";
  import FolderPlusIcon from "@lucide/svelte/icons/folder-plus";
  import CpuIcon from "@lucide/svelte/icons/cpu";
  import PaletteIcon from "@lucide/svelte/icons/palette";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import SlidersIcon from "@lucide/svelte/icons/sliders-horizontal";
  import BookOpenIcon from "@lucide/svelte/icons/book-open";
  import LayersIcon from "@lucide/svelte/icons/layers";
  import CheckIcon from "@lucide/svelte/icons/check";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const NO_SKILL = "__none__";
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

  // Seed local form state from config when the dialog opens (or when config is
  // replaced after a save). Only `open` and `appStore.config` are tracked as
  // triggers; the body is untracked so writing `activeProvider` here (and the
  // `activeProvider` read inside resetKeyState) does NOT make this effect depend
  // on it — otherwise picking a provider would re-run this and clobber the
  // selection back to the config default.
  $effect(() => {
    const isOpen = open;
    const cfg = appStore.config;
    if (!isOpen || !cfg) return;
    untrack(() => {
      // Fall back to the first provider in the list so the UI is never blank.
      activeProvider = cfg.active_provider ?? cfg.providers[0]?.provider ?? "";
      providers = cfg.providers.map((p) => ({
        provider: p.provider,
        model: p.model,
        endpoint: p.endpoint,
        generation: { ...(p.generation ?? {}) },
      }));
      ensureProviderPatch(activeProvider);
      resetKeyState();
      activeSkill = cfg.selected_skill ?? NO_SKILL;
      contextTurns = cfg.max_context_turns ?? 0;
      gen = { ...(cfg.generation ?? {}) };
      loadSkills();
    });
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

  // Ensure a local patch entry exists for `id` so the model field and the
  // per-provider override editor have something to bind to (providers selected
  // from the registry may not yet be in the saved list).
  function ensureProviderPatch(id: string) {
    if (!id || providers.find((p) => p.provider === id)) return;
    const info = appStore.config?.provider_registry?.find((r) => r.id === id);
    providers = [
      ...providers,
      { provider: id, model: "", endpoint: info?.default_endpoint ?? null, generation: {} },
    ];
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
      // Strip empty fields out of each provider's generation overrides.
      const payload = providers.map((p) => ({
        ...p,
        generation: cleanGeneration(p.generation ?? {}),
      }));
      await ipc.setConfig({ providers: payload, active_provider: activeProvider || undefined });
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
      console.error("listModels failed", e);
      modelList = [];
      modelsError = "Couldn't load models — type the name manually.";
    } finally {
      modelsLoading = false;
    }
  }

  // Drop empty/blank fields so a cleared input is omitted (not sent as "" / null,
  // which the Rust Option<T> would reject) — and a legitimate 0 is kept.
  function cleanGeneration(g: GenerationParams): GenerationParams {
    const out: Record<string, unknown> = {};
    for (const [k, v] of Object.entries(g ?? {})) {
      if (v === "" || v == null) continue;
      if (Array.isArray(v) && v.length === 0) continue;
      out[k] = v;
    }
    return out as GenerationParams;
  }

  async function saveGeneration() {
    try {
      await ipc.setConfig({ generation: cleanGeneration(gen) });
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

  // The active provider's per-provider override object (created on demand by
  // ensureProviderPatch). Bound into the per-provider GenerationFields.
  const activeGeneration = $derived(activeProviderPatch()?.generation);
  let showOverrides = $state(false);

  // ── Two-pane nav ──────────────────────────────────────────────────────────
  // Which section the right pane shows. Pure presentation; all form state below
  // is shared across sections and persists when switching.
  type SectionId = "providers" | "theme" | "folders" | "context" | "generation" | "skill";
  let section = $state<SectionId>("providers");

  // Grouped nav: heading → items (id, label, icon component). Rendered in the
  // left sidebar. Order mirrors the previous single-column section order.
  const NAV: { heading: string; items: { id: SectionId; label: string; icon: typeof CpuIcon }[] }[] = [
    { heading: "Models", items: [{ id: "providers", label: "Providers", icon: CpuIcon }] },
    {
      heading: "App",
      items: [
        { id: "theme", label: "Theme", icon: PaletteIcon },
        { id: "folders", label: "Folder access", icon: FolderIcon },
        { id: "context", label: "Context", icon: LayersIcon },
        { id: "generation", label: "Generation", icon: SlidersIcon },
        { id: "skill", label: "Skill", icon: BookOpenIcon },
      ],
    },
  ];

  // Deterministic avatar tint for a provider id: hash → hue, rendered as an
  // oklch background. Stable per id, no hardcoded brand colors.
  function avatarTint(id: string): string {
    let h = 0;
    for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) % 360;
    return `oklch(0.65 0.15 ${h})`;
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-[860px] h-[80vh] p-0 gap-0 overflow-hidden flex flex-row">
    <!-- Left nav -->
    <nav class="flex w-[190px] shrink-0 flex-col border-r border-border bg-sidebar p-3" aria-label="Settings sections">
      <p class="px-2 pb-3 font-display text-sm font-semibold">Settings</p>
      <div class="flex flex-1 flex-col gap-4 overflow-y-auto">
        {#each NAV as group (group.heading)}
          <div class="flex flex-col gap-0.5">
            <p class="px-2 pb-1 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">{group.heading}</p>
            {#each group.items as item (item.id)}
              <button
                type="button"
                aria-current={section === item.id ? "page" : undefined}
                onclick={() => (section = item.id)}
                class="flex items-center gap-2 rounded-md px-2 py-1.5 text-left text-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {section === item.id
                  ? 'bg-accent text-accent-foreground'
                  : 'text-muted-foreground hover:bg-muted/40 hover:text-foreground'}"
              >
                <item.icon class="size-4 shrink-0" />
                {item.label}
              </button>
            {/each}
          </div>
        {/each}
      </div>
      <button
        type="button"
        onclick={() => (open = false)}
        class="mt-3 flex items-center justify-between rounded-md px-2 py-1.5 text-sm text-muted-foreground transition-colors hover:bg-muted/40 hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        Close
        <kbd class="rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[10px]">esc</kbd>
      </button>
    </nav>

    <!-- Right pane -->
    <div class="min-h-0 flex-1 overflow-y-auto p-6">

      <!-- Providers -->
      {#if section === "providers"}
      <div class="space-y-4">
        <div class="space-y-1">
          <h2 class="font-display text-lg font-semibold tracking-tight">Providers</h2>
          <p class="text-sm text-muted-foreground">Choose where zanto gets its intelligence. API keys are stored in your system keychain.</p>
        </div>

        <div class="space-y-1.5">
          <span class="text-xs text-muted-foreground">Active provider</span>
          <div class="flex flex-col gap-2" role="radiogroup" aria-label="Active provider">
            {#each registry as r (r.id)}
              {@const isActive = r.id === activeProvider}
              <button
                type="button"
                role="radio"
                aria-checked={isActive}
                onclick={() => { activeProvider = r.id; ensureProviderPatch(r.id); }}
                class="flex items-center gap-3 rounded-lg border px-3 py-2.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {isActive
                  ? 'border-primary/50 bg-accent/40'
                  : 'border-border hover:bg-muted/40'}"
              >
                <span
                  class="grid size-8 shrink-0 place-items-center rounded-md font-display text-sm font-semibold text-white"
                  style="background: {avatarTint(r.id)}"
                  aria-hidden="true"
                >
                  {r.label.slice(0, 2)}
                </span>
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-sm font-medium text-foreground">{r.label}</span>
                  <span class="block truncate font-mono text-xs text-muted-foreground">{r.default_endpoint ?? "—"}</span>
                </span>
                {#if isActive}
                  <span class="flex items-center gap-1 rounded-full bg-success-soft px-2 py-0.5 font-display text-xs text-success-soft-foreground">
                    <span class="size-1.5 rounded-full bg-success-soft-foreground"></span>
                    Active
                  </span>
                {/if}
              </button>
            {/each}
          </div>
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

          <!-- Per-provider generation overrides -->
          {#if activeGeneration}
            <div class="space-y-2 rounded-md border border-border p-2.5">
              <button
                type="button"
                class="flex w-full items-center gap-1.5 text-xs font-medium text-muted-foreground hover:text-foreground"
                onclick={() => (showOverrides = !showOverrides)}
              >
                <span class="font-mono">{showOverrides ? "▾" : "▸"}</span>
                Generation overrides for {activeProviderLabel}
              </button>
              {#if showOverrides}
                <GenerationFields params={activeGeneration} />
                <p class="text-[10px] text-muted-foreground">
                  Empty fields inherit the global defaults above. Saved with “Save changes”.
                </p>
              {/if}
            </div>
          {/if}
        {/if}

        <Button size="sm" onclick={saveProviders}>Save changes</Button>
      </div>
      {/if}

      <!-- Theme -->
      {#if section === "theme"}
      <div class="space-y-4">
        <div class="space-y-1">
          <h2 class="font-display text-lg font-semibold tracking-tight">Theme</h2>
          <p class="text-sm text-muted-foreground">Pick a color scheme and how dense the layout feels. Changes apply instantly.</p>
        </div>
        <div class="space-y-1.5">
          <span class="text-xs text-muted-foreground" id="cfg-theme-label">Theme</span>
          <div class="grid grid-cols-2 gap-3" role="radiogroup" aria-labelledby="cfg-theme-label">
            {#each [
              { id: "light", name: "Paper", desc: "Bright light theme with a violet accent.", swatch: ["#f7f7f5", "#ffffff", "#ececef", "#6d5ef0"] },
              { id: "dark", name: "Midnight", desc: "Deep dark theme with a violet accent.", swatch: ["#23232b", "#2b2b35", "#34343f", "#8b7cff"] },
            ] as t (t.id)}
              {@const isActive = mode.current === t.id}
              <button
                type="button"
                role="radio"
                aria-checked={isActive}
                onclick={() => setMode(t.id as "light" | "dark")}
                class="flex flex-col gap-2 rounded-lg border p-2.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {isActive
                  ? 'border-primary ring-1 ring-primary/40'
                  : 'border-border hover:bg-muted/40'}"
              >
                <span class="flex h-12 overflow-hidden rounded-md border border-border" aria-hidden="true">
                  {#each t.swatch as c (c)}
                    <span class="flex-1" style="background: {c}"></span>
                  {/each}
                </span>
                <span class="flex items-center justify-between">
                  <span class="text-sm font-medium text-foreground">{t.name}</span>
                  {#if isActive}<CheckIcon class="size-4 text-primary" />{/if}
                </span>
                <span class="text-xs text-muted-foreground">{t.desc}</span>
              </button>
            {/each}
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
      </div>
      {/if}

      <!-- Folder access -->
      {#if section === "folders"}
      <div class="space-y-4">
        <div class="space-y-1">
          <h2 class="font-display text-lg font-semibold tracking-tight">Folder access</h2>
          <p class="text-sm text-muted-foreground">Folders the assistant may read and write. Add one to grant access.</p>
        </div>
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
      </div>
      {/if}

      <!-- Context -->
      {#if section === "context"}
      <div class="space-y-4">
        <div class="space-y-1">
          <h2 class="font-display text-lg font-semibold tracking-tight">Context</h2>
          <p class="text-sm text-muted-foreground">How much conversation history is kept verbatim before older turns are summarized.</p>
        </div>
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
      </div>
      {/if}

      <!-- Generation (global defaults) -->
      {#if section === "generation"}
      <div class="space-y-4">
        <div class="space-y-1">
          <h2 class="font-display text-lg font-semibold tracking-tight">Generation</h2>
          <p class="text-sm text-muted-foreground">Defaults applied to every turn. A provider's overrides take precedence.</p>
        </div>
        <p class="text-xs text-muted-foreground">
          Applied to every turn. A provider's overrides (in Providers) take
          precedence. Empty fields use the provider default; unsupported options are
          ignored per provider.
        </p>
        <GenerationFields bind:params={gen} />
        <Button size="sm" onclick={saveGeneration}>Save generation</Button>
      </div>
      {/if}

      <!-- Skill -->
      {#if section === "skill"}
      <div class="space-y-4">
        <div class="space-y-1">
          <h2 class="font-display text-lg font-semibold tracking-tight">Skill</h2>
          <p class="text-sm text-muted-foreground">Load a markdown skill to steer how the assistant works.</p>
        </div>
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
      </div>
      {/if}
    </div>
  </Dialog.Content>
</Dialog.Root>
