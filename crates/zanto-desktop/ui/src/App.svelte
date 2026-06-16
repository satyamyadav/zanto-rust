<script lang="ts">
  // S3 scaffold placeholder. The real shell (chat panel + right app panel) lands in S8.
  let status = $state("zanto desktop — scaffold OK");
</script>

<main>
  <h1>{status}</h1>
  <p>Tauri 2 + Svelte 5 (plain Vite SPA). Shell UI arrives in S8.</p>
</main>

<style>
  main {
    font-family: system-ui, sans-serif;
    padding: 2rem;
    color: #222;
  }
  h1 {
    font-size: 1.4rem;
  }
</style>
