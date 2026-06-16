<script lang="ts">
  import MessageList from "./MessageList.svelte";
  import Composer from "./Composer.svelte";
  import HitlForm from "./HitlForm.svelte";
</script>

<div class="flex h-full flex-col min-w-0">
  <MessageList />
  <div class="relative">
    <HitlForm />
    <Composer />
  </div>
</div>
