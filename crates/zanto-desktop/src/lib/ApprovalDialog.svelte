<script lang="ts">
  import { onMount } from "svelte";
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { ipc, type ApprovalRequest } from "./ipc";

  let pending = $state<ApprovalRequest | null>(null);

  onMount(() => {
    const un = ipc.onApprovalRequest((r) => (pending = r));
    return () => {
      un.then((f) => f());
    };
  });

  async function respond(r: "once" | "session" | "forever" | "deny") {
    const req = pending;
    if (!req) return;
    pending = null;
    await ipc.approve(req.id, r);
  }
</script>

<Dialog.Root
  open={pending !== null}
  onOpenChange={(o) => {
    if (!o && pending) respond("deny");
  }}
>
  <Dialog.Content class="max-w-md">
    <Dialog.Header>
      <Dialog.Title>Permission required</Dialog.Title>
    </Dialog.Header>
    {#if pending}
      <div class="text-sm">
        <span class="uppercase text-muted-foreground">{pending.op}</span>
        <span class="font-mono">"{pending.path}"</span>
      </div>
      <div class="text-xs text-muted-foreground font-mono break-all">{pending.resolved}</div>
      <Dialog.Footer class="gap-2 sm:justify-start">
        <Button size="sm" onclick={() => respond("once")}>Allow once</Button>
        <Button size="sm" variant="secondary" onclick={() => respond("session")}>Session</Button>
        <Button size="sm" variant="secondary" onclick={() => respond("forever")}>Forever</Button>
        <Button size="sm" variant="destructive" class="sm:ml-auto" onclick={() => respond("deny")}>Deny</Button>
      </Dialog.Footer>
    {/if}
  </Dialog.Content>
</Dialog.Root>
