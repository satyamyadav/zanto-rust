<script lang="ts">
  import { onMount } from "svelte";
  import { ipc, type AppManifest, type ChatBlock } from "$lib/ipc";
  import Block from "$lib/Block.svelte";
  import ApprovalDialog from "$lib/ApprovalDialog.svelte";

  let apps = $state<AppManifest[]>([]);
  let activeId = $state<string | null>(null);
  let input = $state("");
  let convo = $state<ChatBlock[]>([]); // chat thread (inline blocks + markdown)
  let canvas = $state<ChatBlock | null>(null); // right-panel (canvas-targeted / default view)
  let busy = $state(false);

  onMount(async () => {
    apps = await ipc.listApps();
  });

  async function mount(id: string) {
    await ipc.mountApp(id);
    activeId = id;
    canvas = null;
    // Default right-panel view via the manual (ungated) data path.
    try {
      const data = await ipc.queryApp(id, "list_transactions", {});
      canvas = { kind: "component", component_id: "transactions_table", data, target: "canvas" };
    } catch (_) {}
  }

  async function send() {
    const text = input.trim();
    if (!text || busy) return;
    input = "";
    convo.push({ kind: "markdown", text: `> ${text}` });
    busy = true;
    try {
      const turn = await ipc.sendMessage(text);
      for (const b of turn.blocks) {
        if (b.kind === "component" && b.target === "canvas") canvas = b;
        else convo.push(b);
      }
      // Refresh the default table view after a turn (reflects new inserts).
      if (activeId && canvas?.kind === "component" && canvas.component_id === "transactions_table") {
        const data = await ipc.queryApp(activeId, "list_transactions", {});
        canvas = { kind: "component", component_id: "transactions_table", data, target: "canvas" };
      }
    } catch (e) {
      convo.push({ kind: "markdown", text: `Error: ${e}` });
    }
    busy = false;
  }
</script>

<div class="flex h-screen text-gray-900">
  <!-- Left: app nav -->
  <aside class="w-48 border-r border-gray-200 p-3 space-y-1 shrink-0">
    <div class="text-xs uppercase tracking-wide text-gray-400 mb-2">Solutions</div>
    {#each apps as a}
      <button
        class="w-full text-left px-2 py-1.5 rounded text-sm {activeId === a.id ? 'bg-blue-600 text-white' : 'hover:bg-gray-100'}"
        onclick={() => mount(a.id)}
      >
        {a.name}
      </button>
    {/each}
    {#if apps.length === 0}
      <div class="text-sm text-gray-400">No apps</div>
    {/if}
  </aside>

  <!-- Center: chat -->
  <main class="flex-1 flex flex-col min-w-0">
    <div class="flex-1 overflow-auto p-4 space-y-3">
      {#each convo as block}
        <div class="max-w-2xl"><Block {block} /></div>
      {/each}
      {#if busy}<div class="text-sm text-gray-400">…thinking</div>{/if}
    </div>
    <form class="border-t border-gray-200 p-3 flex gap-2" onsubmit={(e) => { e.preventDefault(); send(); }}>
      <input
        class="flex-1 border border-gray-300 rounded px-3 py-2 text-sm"
        placeholder={activeId ? `Ask ${activeId}…` : "Pick a solution to begin…"}
        bind:value={input}
      />
      <button class="px-4 py-2 rounded bg-blue-600 text-white text-sm" type="submit" disabled={busy}>Send</button>
    </form>
  </main>

  <!-- Right: app canvas panel -->
  <section class="w-96 border-l border-gray-200 p-4 overflow-auto shrink-0">
    {#if canvas}
      <Block block={canvas} />
    {:else}
      <div class="text-sm text-gray-400">Mount a solution to see its views.</div>
    {/if}
  </section>
</div>

<ApprovalDialog />
