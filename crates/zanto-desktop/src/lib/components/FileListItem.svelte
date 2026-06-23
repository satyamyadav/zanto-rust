<script lang="ts">
  let { name, path, isDir = false }: { name: string; path: string; isDir?: boolean } = $props();
</script>
<span class="flex min-w-0 flex-1 flex-col">
  <span class="truncate font-mono">{name}{isDir ? "/" : ""}</span>
  <span class="truncate text-xs text-muted-foreground/70 font-mono" title={path}>{path}</span>
</span>
