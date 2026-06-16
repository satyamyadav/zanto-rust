<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { ModeWatcher } from "mode-watcher";
  import { Toaster } from "$lib/components/ui/sonner";
  import { bootstrapDensity } from "$lib/stores/theme.svelte";

  let { children } = $props();

  onMount(() => bootstrapDensity());
</script>

<ModeWatcher />
<Toaster richColors closeButton />

{@render children()}
