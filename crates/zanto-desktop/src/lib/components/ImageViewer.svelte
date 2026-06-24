<script lang="ts">
  import { ipc } from "$lib/ipc";
  import XIcon from "@lucide/svelte/icons/x";
  import ChevronLeftIcon from "@lucide/svelte/icons/chevron-left";
  import ChevronRightIcon from "@lucide/svelte/icons/chevron-right";
  import { onMount, onDestroy } from "svelte";

  // List of image attachments to display. The viewer opens at `activeIndex`.
  let {
    images,
    activeIndex = 0,
    onclose,
  }: {
    images: { path: string; name: string }[];
    activeIndex?: number;
    onclose: () => void;
  } = $props();

  // Intentionally a snapshot of the prop — the viewer manages its own navigation.
  // svelte-ignore state_referenced_locally
  let currentIndex = $state(activeIndex);
  let dataUrl = $state<string | null>(null);
  let loading = $state(true);
  let dialogEl = $state<HTMLElement | null>(null);

  const current = $derived(images[currentIndex]);

  async function loadImage(path: string) {
    loading = true;
    dataUrl = null;
    try {
      dataUrl = await ipc.readImageDataUrl(path);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (current) loadImage(current.path);
  });

  function prev() {
    if (images.length > 1) currentIndex = (currentIndex - 1 + images.length) % images.length;
  }

  function next() {
    if (images.length > 1) currentIndex = (currentIndex + 1) % images.length;
  }

  function onBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") { e.preventDefault(); onclose(); }
    else if (e.key === "ArrowLeft") { e.preventDefault(); prev(); }
    else if (e.key === "ArrowRight") { e.preventDefault(); next(); }
  }

  onMount(() => {
    // Focus the dialog so keyboard events are captured immediately.
    dialogEl?.focus();
    document.addEventListener("keydown", onKeydown);
  });

  onDestroy(() => {
    document.removeEventListener("keydown", onKeydown);
  });
</script>

<!-- Backdrop -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 p-4"
  onclick={onBackdropClick}
  data-image-viewer
>
  <!-- Dialog panel -->
  <div
    bind:this={dialogEl}
    role="dialog"
    aria-modal="true"
    aria-label={current?.name ?? "Image viewer"}
    tabindex="-1"
    class="relative flex max-h-full max-w-full flex-col items-center gap-3 focus:outline-none"
  >
    <!-- Close button -->
    <button
      type="button"
      onclick={onclose}
      aria-label="Close image viewer"
      class="absolute -right-3 -top-3 z-10 flex size-8 items-center justify-center rounded-full bg-background/90 text-foreground shadow-md hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <XIcon class="size-4" />
    </button>

    <!-- Image area -->
    <div class="relative flex max-h-[80vh] max-w-[90vw] items-center justify-center">
      {#if images.length > 1}
        <button
          type="button"
          onclick={prev}
          aria-label="Previous image"
          class="absolute left-2 z-10 flex size-8 items-center justify-center rounded-full bg-background/80 text-foreground shadow-md hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <ChevronLeftIcon class="size-5" />
        </button>
      {/if}

      {#if loading}
        <div class="flex size-32 items-center justify-center text-sm text-white/60">Loading…</div>
      {:else if dataUrl}
        <img
          src={dataUrl}
          alt={current?.name ?? "Image"}
          class="max-h-[80vh] max-w-[90vw] rounded-md object-contain shadow-xl"
          data-viewer-img
        />
      {:else}
        <div class="flex size-32 items-center justify-center text-sm text-white/60">
          Failed to load
        </div>
      {/if}

      {#if images.length > 1}
        <button
          type="button"
          onclick={next}
          aria-label="Next image"
          class="absolute right-2 z-10 flex size-8 items-center justify-center rounded-full bg-background/80 text-foreground shadow-md hover:bg-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <ChevronRightIcon class="size-5" />
        </button>
      {/if}
    </div>

    <!-- Caption: file name + counter -->
    <div class="flex flex-col items-center gap-1">
      <span class="max-w-xs truncate text-sm text-white/80 font-mono">{current?.name ?? ""}</span>
      {#if images.length > 1}
        <span class="text-xs text-white/50">{currentIndex + 1} / {images.length}</span>
      {/if}
    </div>
  </div>
</div>
