<script lang="ts">
  import { AlertCircle, RotateCcw } from "@lucide/svelte";
  import { retry, sessionStore } from "$lib/stores/session.svelte";

  let { message, retryText }: { message: string; retryText: string } = $props();
</script>

<div class="flex flex-col gap-2 rounded-lg border border-destructive/40 bg-destructive/10 p-3 text-xs">
  <div class="flex items-start gap-2 text-destructive">
    <AlertCircle size={14} class="mt-0.5 shrink-0" />
    <span class="whitespace-pre-wrap break-words">{message}</span>
  </div>
  <div>
    <button
      type="button"
      disabled={sessionStore.busy}
      onclick={() => retry(retryText)}
      class="inline-flex items-center gap-1 rounded-md border border-destructive/40 bg-background px-2 py-1 font-medium text-destructive hover:bg-destructive/10 disabled:cursor-not-allowed disabled:opacity-50"
    >
      <RotateCcw size={12} />
      Retry
    </button>
  </div>
</div>
