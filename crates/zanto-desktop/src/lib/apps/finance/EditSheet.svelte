<script lang="ts">
  import type { Snippet } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import X from "@lucide/svelte/icons/x";

  let {
    open = $bindable(false),
    title,
    children,
    onSave,
    saveLabel = "Save",
    canSave = true,
    saving = false,
  }: {
    open?: boolean;
    title: string;
    children: Snippet;
    onSave?: () => void;
    saveLabel?: string;
    canSave?: boolean;
    saving?: boolean;
  } = $props();

  function close() {
    open = false;
  }
</script>

{#if open}
  <!-- Backdrop -->
  <div
    class="fixed inset-0 z-40 bg-black/40"
    role="presentation"
    onclick={close}
  ></div>

  <!-- Right sheet -->
  <div
    class="fixed inset-y-0 right-0 z-50 flex w-[360px] max-w-[90vw] flex-col border-l border-border bg-card shadow-xl"
    role="dialog"
    aria-modal="true"
    aria-label={title}
  >
    <div class="flex items-center justify-between border-b border-border px-4 py-3">
      <div class="font-display text-sm font-semibold">{title}</div>
      <button
        type="button"
        class="rounded-sm text-muted-foreground outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring"
        onclick={close}
        aria-label="Close"
      >
        <X class="size-4" />
      </button>
    </div>

    <div class="min-h-0 flex-1 overflow-y-auto p-4">
      {@render children()}
    </div>

    <div class="flex items-center justify-end gap-2 border-t border-border px-4 py-3">
      <Button variant="ghost" size="sm" onclick={close} disabled={saving}>Cancel</Button>
      {#if onSave}
        <Button size="sm" onclick={onSave} disabled={!canSave || saving}>
          {saving ? "Saving…" : saveLabel}
        </Button>
      {/if}
    </div>
  </div>
{/if}
