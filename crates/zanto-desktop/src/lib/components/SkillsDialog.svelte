<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Textarea } from "$lib/components/ui/textarea";
  import { toast } from "svelte-sonner";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import Trash2Icon from "@lucide/svelte/icons/trash-2";
  import PencilIcon from "@lucide/svelte/icons/pencil";
  import BookOpenIcon from "@lucide/svelte/icons/book-open";
  import { ipc, type SkillDto, type SkillScope } from "$lib/ipc";
  import { appStore } from "$lib/stores/app.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  // The editor manages one scope's dir at a time; toggle switches the list and
  // the dir written to. Project scope needs an active project.
  let scope = $state<SkillScope>("global");
  const hasProject = $derived(!!appStore.config?.project_dir);

  let skills = $state<SkillDto[]>([]);
  // The skill currently open in the editor. `null` = nothing selected; a draft
  // with `isNew` is an unsaved new skill (name still editable).
  let selectedName = $state<string | null>(null);
  let editorName = $state("");
  let editorBody = $state("");
  let isNew = $state(false);
  let dirty = $state(false);
  let busy = $state(false);

  // Skills for the active scope (server already filters per scope on list).
  const scopeSkills = $derived(skills.filter((s) => s.scope === scope));

  async function refresh() {
    try {
      skills = await ipc.listSkills();
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  // Reload the list each time the dialog opens; reset the editor.
  $effect(() => {
    if (open) {
      void refresh();
      closeEditor();
      // Default to a usable scope: global always works; project only with a project.
      if (!hasProject) scope = "global";
    }
  });

  function closeEditor() {
    selectedName = null;
    editorName = "";
    editorBody = "";
    isNew = false;
    dirty = false;
  }

  async function selectSkill(name: string) {
    if (busy) return;
    busy = true;
    try {
      const body = await ipc.readSkill(name, scope);
      selectedName = name;
      editorName = name;
      editorBody = body;
      isNew = false;
      dirty = false;
    } catch (e) {
      toast.error(`${e}`);
    } finally {
      busy = false;
    }
  }

  function newSkill() {
    selectedName = null;
    editorName = "";
    editorBody = "";
    isNew = true;
    dirty = true;
  }

  async function save() {
    const name = editorName.trim();
    if (!name) {
      toast.error("Give the skill a name");
      return;
    }
    busy = true;
    try {
      const dto = await ipc.saveSkill(name, scope, editorBody);
      await refresh();
      selectedName = dto.name;
      editorName = dto.name;
      isNew = false;
      dirty = false;
      toast.success(`Saved “${dto.name}”`);
    } catch (e) {
      toast.error(`${e}`);
    } finally {
      busy = false;
    }
  }

  async function remove(name: string) {
    busy = true;
    try {
      await ipc.deleteSkill(name, scope);
      await refresh();
      if (selectedName === name) closeEditor();
      toast.success(`Deleted “${name}”`);
    } catch (e) {
      toast.error(`${e}`);
    } finally {
      busy = false;
    }
  }

  // Rename via a prompt — minimal v1 (the editor's name field saves a NEW file;
  // renaming the file itself is an explicit action so it can't be confused with
  // "save as").
  async function rename(name: string) {
    const next = window.prompt(`Rename skill “${name}” to:`, name);
    if (next === null) return;
    const trimmed = next.trim();
    if (!trimmed || trimmed === name) return;
    busy = true;
    try {
      await ipc.renameSkill(name, trimmed, scope);
      await refresh();
      if (selectedName === name) {
        selectedName = trimmed;
        editorName = trimmed;
      }
      toast.success(`Renamed to “${trimmed}”`);
    } catch (e) {
      toast.error(`${e}`);
    } finally {
      busy = false;
    }
  }

  function setScope(next: SkillScope) {
    if (next === scope) return;
    scope = next;
    closeEditor();
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-[860px] h-[80vh] p-0 gap-0 overflow-hidden flex flex-row">
    <!-- Left: scope toggle + skill list -->
    <nav class="flex w-[260px] shrink-0 flex-col border-r border-border bg-sidebar p-3" aria-label="Skills">
      <div class="flex items-center justify-between px-1 pb-3">
        <p class="font-display text-sm font-semibold">Skills</p>
        <Button size="sm" variant="ghost" class="h-7 gap-1 px-2 text-xs" onclick={newSkill} disabled={busy}>
          <PlusIcon class="size-3.5" />
          New
        </Button>
      </div>

      <!-- Scope toggle -->
      <div class="mb-3 flex gap-1 rounded-md bg-muted/50 p-0.5 text-xs">
        <button
          type="button"
          onclick={() => setScope("project")}
          disabled={!hasProject}
          aria-pressed={scope === "project"}
          title={hasProject ? "Project skills (.zanto/skills)" : "Set a project to manage project skills"}
          class="flex-1 rounded px-2 py-1 transition-colors disabled:opacity-40 {scope === 'project'
            ? 'bg-background font-medium text-foreground shadow-sm'
            : 'text-muted-foreground hover:text-foreground'}"
        >
          Project
        </button>
        <button
          type="button"
          onclick={() => setScope("global")}
          aria-pressed={scope === "global"}
          title="Global skills (shared across projects)"
          class="flex-1 rounded px-2 py-1 transition-colors {scope === 'global'
            ? 'bg-background font-medium text-foreground shadow-sm'
            : 'text-muted-foreground hover:text-foreground'}"
        >
          Global
        </button>
      </div>

      <div class="flex flex-1 flex-col gap-0.5 overflow-y-auto">
        {#if scope === "project" && !hasProject}
          <p class="px-2 py-4 text-xs text-muted-foreground">
            Set a project (Workspace) to create project-local skills, or use Global.
          </p>
        {:else if scopeSkills.length === 0}
          <p class="px-2 py-4 text-xs text-muted-foreground">
            No {scope} skills yet. Click <span class="font-medium">New</span> to create one.
          </p>
        {:else}
          {#each scopeSkills as s (s.name)}
            <div
              class="group flex items-center gap-1 rounded-md px-2 py-1.5 text-sm transition-colors {selectedName === s.name
                ? 'bg-accent text-accent-foreground'
                : 'text-muted-foreground hover:bg-muted/40 hover:text-foreground'}"
            >
              <button
                type="button"
                onclick={() => selectSkill(s.name)}
                class="flex min-w-0 flex-1 items-center gap-2 text-left focus-visible:outline-none"
              >
                <BookOpenIcon class="size-3.5 shrink-0" />
                <span class="truncate font-mono">{s.name}</span>
              </button>
              <button
                type="button"
                onclick={() => rename(s.name)}
                aria-label="Rename {s.name}"
                title="Rename"
                class="shrink-0 rounded p-0.5 opacity-0 hover:text-foreground group-hover:opacity-100"
              >
                <PencilIcon class="size-3.5" />
              </button>
              <button
                type="button"
                onclick={() => remove(s.name)}
                aria-label="Delete {s.name}"
                title="Delete"
                class="shrink-0 rounded p-0.5 opacity-0 hover:text-destructive group-hover:opacity-100"
              >
                <Trash2Icon class="size-3.5" />
              </button>
            </div>
          {/each}
        {/if}
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

    <!-- Right: editor -->
    <div class="flex min-h-0 flex-1 flex-col p-6">
      {#if selectedName === null && !isNew}
        <div class="flex flex-1 flex-col items-center justify-center gap-2 text-center text-muted-foreground">
          <BookOpenIcon class="size-8 opacity-40" />
          <p class="text-sm">Select a skill to edit, or create a new one.</p>
          <p class="max-w-sm text-xs">
            A skill is a markdown preprompt appended to the system prompt when you pick it in the
            composer.
          </p>
        </div>
      {:else}
        <div class="mb-3 space-y-1">
          <label for="skill-name" class="text-xs text-muted-foreground">Name</label>
          <Input
            id="skill-name"
            bind:value={editorName}
            oninput={() => (dirty = true)}
            readonly={!isNew}
            placeholder="my-skill"
            class="font-mono {!isNew ? 'opacity-70' : ''}"
          />
          {#if !isNew}
            <p class="text-[11px] text-muted-foreground">Use Rename (pencil) to change the file name.</p>
          {/if}
        </div>
        <div class="flex min-h-0 flex-1 flex-col space-y-1">
          <label for="skill-body" class="text-xs text-muted-foreground">Skill (markdown)</label>
          <Textarea
            id="skill-body"
            bind:value={editorBody}
            oninput={() => (dirty = true)}
            placeholder="You are a meticulous code reviewer. Focus on correctness and clarity…"
            class="min-h-0 flex-1 resize-none font-mono text-sm"
          />
        </div>
        <div class="mt-3 flex items-center justify-end gap-2">
          <Button variant="ghost" onclick={closeEditor} disabled={busy}>Cancel</Button>
          <Button onclick={save} disabled={busy || !dirty || !editorName.trim()}>
            {isNew ? "Create" : "Save"}
          </Button>
        </div>
      {/if}
    </div>
  </Dialog.Content>
</Dialog.Root>
