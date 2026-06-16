<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { toast } from "svelte-sonner";
  import { mode, setMode } from "mode-watcher";
  import { density, setDensity, type Density } from "$lib/stores/theme.svelte";
  import { appStore, refreshConfig } from "$lib/stores/app.svelte";
  import { ipc } from "$lib/ipc";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  let model = $state("");
  let endpoint = $state("");

  $effect(() => {
    if (open && appStore.config) {
      model = appStore.config.model;
      endpoint = appStore.config.endpoint;
    }
  });

  async function saveModel() {
    try {
      await ipc.setConfig({ model, endpoint });
      await refreshConfig();
      toast.success("Settings saved");
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  async function pickFolder() {
    try {
      const f = await ipc.pickFolder();
      if (f) toast.message(`Granted: ${f}`, { description: "Restart to apply folder access." });
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  const densities: Density[] = ["compact", "normal", "relaxed"];
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-lg">
    <Dialog.Header>
      <Dialog.Title>Settings</Dialog.Title>
    </Dialog.Header>

    <div class="space-y-6 py-1">
      <section class="space-y-2">
        <h3 class="text-sm font-medium">Model</h3>
        <div class="space-y-1">
          <label class="text-xs text-muted-foreground" for="cfg-model">Model</label>
          <Input id="cfg-model" bind:value={model} placeholder="gemini-flash-latest" />
        </div>
        <div class="space-y-1">
          <label class="text-xs text-muted-foreground" for="cfg-endpoint">Endpoint</label>
          <Input id="cfg-endpoint" bind:value={endpoint} placeholder="http://localhost:11434/" />
        </div>
        <p class="text-[11px] text-muted-foreground">API keys are read from the environment (e.g. GEMINI_API_KEY).</p>
        <Button size="sm" onclick={saveModel}>Save</Button>
      </section>

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
