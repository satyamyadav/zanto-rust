<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { toast } from "svelte-sonner";
  import { appStore, refreshConfig } from "$lib/stores/app.svelte";
  import { ipc } from "$lib/ipc";
  import FolderPlusIcon from "@lucide/svelte/icons/folder-plus";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import XIcon from "@lucide/svelte/icons/x";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  const projectDir = $derived(appStore.config?.project_dir ?? null);
  const sources = $derived(appStore.config?.context_sources ?? []);

  // Output location note: artifacts land under the project's `.zanto/artifacts`,
  // or the global store when no project is set.
  const outputPath = $derived(
    projectDir ? `${projectDir}/.zanto/artifacts` : "the global store",
  );

  async function setProject() {
    try {
      const f = await ipc.pickFolder();
      if (!f) return;
      await ipc.setProjectDir(f);
      await refreshConfig();
      toast.success("Project set", { description: f });
    } catch (e) {
      toast.error("Could not set the project", { description: `${e}` });
    }
  }

  async function addSource() {
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

  async function toggleSource(path: string, enabled: boolean) {
    try {
      await ipc.toggleContextSource(path, enabled);
      await refreshConfig();
    } catch (e) {
      toast.error("Could not update the context source", { description: `${e}` });
    }
  }

  async function removeSource(path: string) {
    try {
      await ipc.removeContextSource(path);
      await refreshConfig();
      toast.success("Context source removed", { description: path });
    } catch (e) {
      toast.error("Could not remove the context source", { description: `${e}` });
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-w-lg">
    <Dialog.Header>
      <Dialog.Title class="font-display">Workspace</Dialog.Title>
    </Dialog.Header>

    <div class="space-y-6 py-1">
      <!-- Project (output + read) -->
      <section class="space-y-3">
        <div>
          <h3 class="font-display text-sm font-semibold tracking-tight">Project</h3>
          <p class="text-xs text-muted-foreground">
            Where outputs are written and read. Saved artifacts land in
            <code class="font-mono">{outputPath}</code>.
          </p>
        </div>
        {#if projectDir}
          <div class="flex items-center gap-2 rounded-md bg-muted px-2.5 py-1.5">
            <FolderIcon class="size-4 shrink-0 text-muted-foreground" />
            <span class="flex-1 truncate font-mono text-xs text-foreground" title={projectDir}>
              {projectDir}
            </span>
          </div>
        {:else}
          <p class="text-xs text-muted-foreground">
            No project — outputs go to the global store.
          </p>
        {/if}
        <Button size="sm" variant="outline" onclick={setProject}>
          <FolderPlusIcon class="size-3.5" />
          {projectDir ? "Change project…" : "Set project…"}
        </Button>
      </section>

      <!-- Context sources (inputs) -->
      <section class="space-y-3">
        <div>
          <h3 class="font-display text-sm font-semibold tracking-tight">Context sources</h3>
          <p class="text-xs text-muted-foreground">
            Files and folders fed to every turn. Toggle to silence a source without removing it.
          </p>
        </div>
        {#if sources.length === 0}
          <p class="text-xs text-muted-foreground">
            No context sources yet. Add a folder of notes to inject into every turn.
          </p>
        {:else}
          <ul class="space-y-1">
            {#each sources as src (src.path)}
              <li class="flex items-center gap-2 rounded-md bg-muted px-2.5 py-1.5">
                <button
                  type="button"
                  role="switch"
                  aria-checked={src.enabled}
                  aria-label={src.enabled ? "Disable source" : "Enable source"}
                  title={src.enabled ? "Enabled — click to disable" : "Disabled — click to enable"}
                  onclick={() => toggleSource(src.path, !src.enabled)}
                  class="relative inline-flex h-4 w-7 shrink-0 items-center rounded-full transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring {src.enabled
                    ? 'bg-primary'
                    : 'bg-input'}"
                >
                  <span
                    class="inline-block size-3 rounded-full bg-background shadow transition-transform {src.enabled
                      ? 'translate-x-3.5'
                      : 'translate-x-0.5'}"
                  ></span>
                </button>
                <span
                  class="flex-1 truncate font-mono text-xs {src.enabled
                    ? 'text-foreground'
                    : 'text-muted-foreground line-through'}"
                  title={src.path}
                >
                  {src.path}
                </span>
                <button
                  type="button"
                  class="grid size-5 place-items-center rounded text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                  onclick={() => removeSource(src.path)}
                  aria-label="Remove context source"
                >
                  <XIcon class="size-3.5" />
                </button>
              </li>
            {/each}
          </ul>
        {/if}
        <Button size="sm" variant="outline" onclick={addSource}>
          <FolderPlusIcon class="size-3.5" />
          Add source…
        </Button>
      </section>
    </div>
  </Dialog.Content>
</Dialog.Root>
