<script lang="ts">
  let { data }: { data: any } = $props();
  const text = $derived(JSON.stringify(data?.value ?? data, null, 2));
</script>

<pre class="text-xs bg-muted p-2 rounded overflow-auto">{text}</pre>
