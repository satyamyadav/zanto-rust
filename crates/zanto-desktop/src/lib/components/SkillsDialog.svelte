<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Textarea } from "$lib/components/ui/textarea";
  import { toast } from "svelte-sonner";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import Trash2Icon from "@lucide/svelte/icons/trash-2";
  import PencilIcon from "@lucide/svelte/icons/pencil";
  import BookOpenIcon from "@lucide/svelte/icons/book-open";
  import { ipc, type SkillDto, type SkillScope } from "$lib/ipc";
  import { appStore } from "$lib/stores/app.svelte";

  let { open = $bindable(false) }: { open?: boolean } = $props();

  // Markdown skeleton a new skill starts from, so the editor isn't a blank box.
  const TEMPLATE = `# Skill

Describe what this skill makes the assistant do.

## Voice
-

## Focus
-

## Avoid
-
`;

  // The editor manages one scope's dir at a time; toggle switches the list and
  // the dir written to. Project scope needs an active project.
  let scope = $state<SkillScope>("global");
  const hasProject = $derived(!!appStore.config?.project_dir);

  let skills = $state<SkillDto[]>([]);
  // The skill currently open in the editor: a saved skill's name, or `null` when
  // a draft (unsaved new skill) is being edited / nothing is selected.
  let selectedName = $state<string | null>(null);
  let editorBody = $state("");
  let dirty = $state(false);
  let busy = $state(false);

  // Unsaved new skill. When non-null it renders as the top row of the list
  // ("untitled" by default), name editable in place, body = TEMPLATE.
  let draft = $state<{ name: string } | null>(null);

  // Inline rename: the name currently being edited in the list (for a saved
  // skill), and the working text. `null` = no inline rename in progress.
  let renaming = $state<string | null>(null);
  let renameText = $state("");

  const scopeSkills = $derived(skills.filter((s) => s.scope === scope));
  const hasEditor = $derived(draft !== null || selectedName !== null);

  async function refresh() {
    try {
      skills = await ipc.listSkills();
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  // Reload the list and reset editor state ONLY on the rising edge of `open`
  // (the dialog actually opening) — not on every dependency change while it's
  // open. Without this latch, the body also read `hasProject`, so a late config
  // load (project_dir resolving after the dialog opened) re-ran resetEditor()
  // mid-edit and silently discarded an unsaved draft/edit.
  let wasOpen = false;
  $effect(() => {
    const isOpen = open;
    if (isOpen && !wasOpen) {
      void refresh();
      resetEditor();
      if (!hasProject) scope = "global";
    }
    wasOpen = isOpen;
  });

  function resetEditor() {
    selectedName = null;
    editorBody = "";
    draft = null;
    dirty = false;
    renaming = null;
  }

  async function selectSkill(name: string) {
    if (busy || renaming === name) return;
    busy = true;
    try {
      const body = await ipc.readSkill(name, scope);
      draft = null;
      selectedName = name;
      editorBody = body;
      dirty = false;
    } catch (e) {
      toast.error(`${e}`);
    } finally {
      busy = false;
    }
  }

  // Start a new skill: an "untitled" draft row at the top, selected, with the
  // template loaded into the editor.
  function newSkill() {
    renaming = null;
    selectedName = null;
    draft = { name: "untitled" };
    editorBody = TEMPLATE;
    dirty = true;
  }

  async function save() {
    const name = draft ? draft.name.trim() : selectedName;
    if (!name) {
      toast.error("Give the skill a name");
      return;
    }
    busy = true;
    try {
      // A draft is a NEW skill (overwrite=false → backend refuses to clobber an
      // existing name); saving an opened existing skill replaces its own file.
      const overwrite = draft === null;
      const dto = await ipc.saveSkill(name, scope, editorBody, overwrite);
      await refresh();
      draft = null;
      selectedName = dto.name;
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
      if (selectedName === name) resetEditor();
      toast.success(`Deleted “${name}”`);
    } catch (e) {
      toast.error(`${e}`);
    } finally {
      busy = false;
    }
  }

  // ── Inline rename ─────────────────────────────────────────────────────────
  function startRename(name: string) {
    draft = null;
    renaming = name;
    renameText = name;
  }

  function cancelRename() {
    renaming = null;
    renameText = "";
  }

  async function commitRename() {
    const from = renaming;
    if (from === null) return;
    const to = renameText.trim();
    renaming = null;
    if (!to || to === from) return;
    busy = true;
    try {
      await ipc.renameSkill(from, to, scope);
      await refresh();
      if (selectedName === from) selectedName = to;
      toast.success(`Renamed to “${to}”`);
    } catch (e) {
      toast.error(`${e}`);
    } finally {
      busy = false;
    }
  }

  function onRenameKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      void commitRename();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelRename();
    }
  }

  function setScope(next: SkillScope) {
    if (next === scope) return;
    scope = next;
    resetEditor();
  }

  // Tab inserts two spaces instead of moving focus, so the raw markdown editor is
  // usable for indented lists/code without leaving the textarea.
  function onBodyKey(e: KeyboardEvent) {
    if (e.key !== "Tab") return;
    e.preventDefault();
    const ta = e.currentTarget as HTMLTextAreaElement;
    const { selectionStart: s, selectionEnd: en } = ta;
    editorBody = editorBody.slice(0, s) + "  " + editorBody.slice(en);
    dirty = true;
    queueMicrotask(() => ta.setSelectionRange(s + 2, s + 2));
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
        <!-- Unsaved new-skill row, pinned to the top with its name editable. -->
        {#if draft}
          <div class="flex items-center gap-2 rounded-md bg-accent px-2 py-1.5 text-sm text-accent-foreground">
            <BookOpenIcon class="size-3.5 shrink-0" />
            <!-- svelte-ignore a11y_autofocus -->
            <input
              autofocus
              bind:value={draft.name}
              oninput={() => (dirty = true)}
              placeholder="untitled"
              aria-label="New skill name"
              class="min-w-0 flex-1 bg-transparent font-mono outline-none placeholder:text-muted-foreground"
            />
          </div>
        {/if}

        {#if scope === "project" && !hasProject}
          <p class="px-2 py-4 text-xs text-muted-foreground">
            Set a project (Workspace) to create project-local skills, or use Global.
          </p>
        {:else if scopeSkills.length === 0 && !draft}
          <p class="px-2 py-4 text-xs text-muted-foreground">
            No {scope} skills yet. Click <span class="font-medium">New</span> to create one.
          </p>
        {:else}
          {#each scopeSkills as s (s.name)}
            <div
              class="group flex items-center gap-1 rounded-md px-2 py-1.5 text-sm transition-colors {selectedName === s.name && !draft
                ? 'bg-accent text-accent-foreground'
                : 'text-muted-foreground hover:bg-muted/40 hover:text-foreground'}"
            >
              {#if renaming === s.name}
                <BookOpenIcon class="size-3.5 shrink-0" />
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  autofocus
                  bind:value={renameText}
                  onkeydown={onRenameKey}
                  onblur={commitRename}
                  aria-label="Rename {s.name}"
                  class="min-w-0 flex-1 bg-transparent font-mono outline-none"
                />
              {:else}
                <button
                  type="button"
                  onclick={() => selectSkill(s.name)}
                  ondblclick={() => startRename(s.name)}
                  class="flex min-w-0 flex-1 items-center gap-2 text-left focus-visible:outline-none"
                >
                  <BookOpenIcon class="size-3.5 shrink-0" />
                  <span class="truncate font-mono">{s.name}</span>
                </button>
                <button
                  type="button"
                  onclick={() => startRename(s.name)}
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
              {/if}
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

    <!-- Right: raw markdown editor -->
    <div class="flex min-h-0 flex-1 flex-col p-6">
      {#if !hasEditor}
        <div class="flex flex-1 flex-col items-center justify-center gap-2 text-center text-muted-foreground">
          <BookOpenIcon class="size-8 opacity-40" />
          <p class="text-sm">Select a skill to edit, or create a new one.</p>
          <p class="max-w-sm text-xs">
            A skill is a markdown preprompt appended to the system prompt when you pick it in the
            composer.
          </p>
        </div>
      {:else}
        <div class="flex min-h-0 flex-1 flex-col space-y-1">
          <label for="skill-body" class="text-xs text-muted-foreground">
            {draft ? "New skill" : selectedName} — markdown
          </label>
          <Textarea
            id="skill-body"
            bind:value={editorBody}
            oninput={() => (dirty = true)}
            onkeydown={onBodyKey}
            placeholder="You are a meticulous code reviewer. Focus on correctness and clarity…"
            class="min-h-0 flex-1 resize-none font-mono text-sm leading-relaxed"
          />
        </div>
        <div class="mt-3 flex items-center justify-end gap-2">
          <Button variant="ghost" onclick={resetEditor} disabled={busy}>Cancel</Button>
          <Button onclick={save} disabled={busy || !dirty || (draft ? !draft.name.trim() : false)}>
            {draft ? "Create" : "Save"}
          </Button>
        </div>
      {/if}
    </div>
  </Dialog.Content>
</Dialog.Root>
