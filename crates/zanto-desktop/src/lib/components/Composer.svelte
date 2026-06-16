<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Textarea } from "$lib/components/ui/textarea";
  import { toast } from "svelte-sonner";
  import SendIcon from "@lucide/svelte/icons/send";
  import PaperclipIcon from "@lucide/svelte/icons/paperclip";
  import XIcon from "@lucide/svelte/icons/x";
  import { sessionStore, send } from "$lib/stores/session.svelte";
  import { appStore } from "$lib/stores/app.svelte";

  // Large pastes become collapsed chips instead of flooding the textarea; the
  // full text is still spliced into the final message on send.
  const CHAR_THRESHOLD = 2000;
  const LINE_THRESHOLD = 20;

  type Paste = { id: number; text: string; lines: number };

  let input = $state("");
  let pastes = $state<Paste[]>([]);
  let nextId = 0;

  function lineCount(text: string): number {
    return text.split("\n").length;
  }

  function isLarge(text: string): boolean {
    return text.length > CHAR_THRESHOLD || lineCount(text) > LINE_THRESHOLD;
  }

  function onpaste(e: ClipboardEvent) {
    const text = e.clipboardData?.getData("text/plain") ?? "";
    if (!isLarge(text)) return; // small pastes behave normally
    e.preventDefault();
    pastes = [...pastes, { id: nextId++, text, lines: lineCount(text) }];
  }

  function removePaste(id: number) {
    pastes = pastes.filter((p) => p.id !== id);
  }

  function composeMessage(): string {
    const typed = input.trim();
    const attached = pastes.map((p) => p.text).join("\n\n");
    return [typed, attached].filter((s) => s.length > 0).join("\n\n");
  }

  async function submit() {
    if (sessionStore.busy) return;
    const text = composeMessage();
    if (!text) return;
    input = "";
    pastes = [];
    try {
      await send(text);
    } catch (e) {
      toast.error(`${e}`);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }
</script>

<form
  class="border-t border-border p-3 flex flex-col gap-2"
  onsubmit={(e) => {
    e.preventDefault();
    submit();
  }}
>
  {#if pastes.length > 0}
    <div class="flex flex-wrap gap-1.5">
      {#each pastes as p (p.id)}
        <span
          class="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted px-2 py-1 text-xs text-muted-foreground"
        >
          <PaperclipIcon class="size-3.5" />
          pasted {p.lines} {p.lines === 1 ? "line" : "lines"}
          <button
            type="button"
            onclick={() => removePaste(p.id)}
            aria-label="Remove pasted text"
            class="rounded hover:text-foreground"
          >
            <XIcon class="size-3.5" />
          </button>
        </span>
      {/each}
    </div>
  {/if}
  <div class="flex items-end gap-2">
    <Textarea
      bind:value={input}
      {onkeydown}
      {onpaste}
      rows={2}
      placeholder={appStore.activeId ? `Ask ${appStore.activeId}…` : "Pick a solution to begin…"}
      class="resize-none"
    />
    <Button type="submit" size="icon" disabled={sessionStore.busy}>
      <SendIcon class="size-4" />
    </Button>
  </div>
</form>
