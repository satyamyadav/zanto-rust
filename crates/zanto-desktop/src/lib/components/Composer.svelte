<script lang="ts">
  import { Button } from "$lib/components/ui/button";
  import { Textarea } from "$lib/components/ui/textarea";
  import { toast } from "svelte-sonner";
  import SendIcon from "@lucide/svelte/icons/send";
  import { sessionStore, send } from "$lib/stores/session.svelte";
  import { appStore } from "$lib/stores/app.svelte";

  let input = $state("");

  async function submit() {
    const text = input.trim();
    if (!text || sessionStore.busy) return;
    input = "";
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
  class="border-t border-border p-3 flex items-end gap-2"
  onsubmit={(e) => {
    e.preventDefault();
    submit();
  }}
>
  <Textarea
    bind:value={input}
    {onkeydown}
    rows={2}
    placeholder={appStore.activeId ? `Ask ${appStore.activeId}…` : "Pick a solution to begin…"}
    class="resize-none"
  />
  <Button type="submit" size="icon" disabled={sessionStore.busy}>
    <SendIcon class="size-4" />
  </Button>
</form>
