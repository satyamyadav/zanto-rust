<script lang="ts">
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { confirmStore, resolveConfirm } from "$lib/stores/confirm.svelte";

  const pending = $derived(confirmStore.pending);

  // Dismissing the dialog (overlay/escape/close) counts as cancel.
  function onOpenChange(open: boolean) {
    if (!open && pending) resolveConfirm(false);
  }
</script>

<Dialog.Root open={!!pending} {onOpenChange}>
  <Dialog.Content class="sm:max-w-sm">
    <Dialog.Header>
      <Dialog.Title class="font-display">{pending?.title ?? "Are you sure?"}</Dialog.Title>
      {#if pending?.body}
        <Dialog.Description>{pending.body}</Dialog.Description>
      {/if}
    </Dialog.Header>
    <Dialog.Footer>
      <Button variant="ghost" onclick={() => resolveConfirm(false)}>
        {pending?.cancelLabel ?? "Cancel"}
      </Button>
      <Button
        variant={pending?.destructive ? "destructive" : "default"}
        onclick={() => resolveConfirm(true)}
      >
        {pending?.confirmLabel ?? "Confirm"}
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>
