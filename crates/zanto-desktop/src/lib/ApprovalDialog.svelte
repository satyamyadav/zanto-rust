<script lang="ts">
  import { onMount } from "svelte";
  import { ipc, type ApprovalRequest } from "./ipc";

  let pending = $state<ApprovalRequest | null>(null);

  onMount(() => {
    const un = ipc.onApprovalRequest((r) => (pending = r));
    return () => {
      un.then((f) => f());
    };
  });

  async function respond(resp: "once" | "session" | "forever" | "deny") {
    const req = pending;
    if (!req) return;
    pending = null;
    await ipc.approve(req.id, resp);
  }
</script>

{#if pending}
  <div class="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
    <div class="bg-white rounded-lg shadow-xl p-5 w-[28rem] space-y-3">
      <div class="font-medium">Permission required</div>
      <div class="text-sm">
        <span class="uppercase text-gray-500">{pending.op}</span>
        <span class="font-mono">"{pending.path}"</span>
      </div>
      <div class="text-xs text-gray-500 font-mono break-all">{pending.resolved}</div>
      <div class="flex gap-2 pt-2">
        <button class="px-3 py-1 rounded bg-blue-600 text-white text-sm" onclick={() => respond("once")}>
          Allow once
        </button>
        <button class="px-3 py-1 rounded bg-gray-200 text-sm" onclick={() => respond("session")}>
          Session
        </button>
        <button class="px-3 py-1 rounded bg-gray-200 text-sm" onclick={() => respond("forever")}>
          Forever
        </button>
        <button class="px-3 py-1 rounded bg-red-100 text-red-700 text-sm ml-auto" onclick={() => respond("deny")}>
          Deny
        </button>
      </div>
    </div>
  </div>
{/if}
