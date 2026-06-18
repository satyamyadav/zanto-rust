<script lang="ts">
  import { tick } from "svelte";
  import { ArrowDown, MessagesSquare, Square, X } from "@lucide/svelte";
  import Message from "./Message.svelte";
  import { sessionStore, loadOlder, removeQueued } from "$lib/stores/session.svelte";

  // The trailing entry is the only one that can be the live, streaming turn;
  // pass a flag down so Message lights the agent-spine for the active turn.
  const lastId = $derived(sessionStore.convo.at(-1)?.id);

  let scroller: HTMLDivElement;
  // True while the viewport is parked at (or near) the bottom. Drives both the
  // autoscroll pin and the visibility of the "jump to latest" affordance.
  let atBottom = $state(true);

  const NEAR_BOTTOM_PX = 48;
  // Trigger an older-page fetch when scrolled within this many px of the top.
  const NEAR_TOP_PX = 64;

  function isAtBottom() {
    if (!scroller) return true;
    return scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight <= NEAR_BOTTOM_PX;
  }

  function scrollToBottom() {
    if (scroller) scroller.scrollTop = scroller.scrollHeight;
  }

  // Pull older history when the user nears the top, anchoring the viewport so the
  // prepended messages don't yank the scroll position. We pin to the distance
  // from the bottom (scrollHeight - scrollTop), which is invariant under a
  // prepend, then restore it once the new content has laid out.
  async function maybeLoadOlder() {
    if (!scroller || !sessionStore.hasMore || sessionStore.loadingOlder) return;
    // Don't pull history while pinned at the bottom: the autoscroll effect owns
    // the scroll position there, and on a short (non-scrollable) thread top and
    // bottom coincide — fetching would fight the autoscroll.
    if (atBottom) return;
    if (scroller.scrollTop > NEAR_TOP_PX) return;
    const fromBottom = scroller.scrollHeight - scroller.scrollTop;
    await loadOlder();
    await tick();
    if (scroller) scroller.scrollTop = scroller.scrollHeight - fromBottom;
  }

  function onScroll() {
    atBottom = isAtBottom();
    void maybeLoadOlder();
  }

  // Autoscroll on new entries/segments/stream activity, but only while the user
  // is already pinned to the bottom — never yank them down mid-scrollback. Wait
  // for the DOM to lay out the new content (tick) so scrollHeight is current,
  // then re-sync atBottom against the post-scroll geometry.
  $effect(() => {
    sessionStore.convo.length;
    sessionStore.convo.at(-1)?.segments.length;
    sessionStore.busy;
    sessionStore.streaming;
    sessionStore.queue.length;
    if (!atBottom) return;
    tick().then(() => {
      scrollToBottom();
      atBottom = isAtBottom();
    });
  });
</script>

<div class="relative min-h-0 flex-1">
  <div
    bind:this={scroller}
    onscroll={onScroll}
    class="absolute inset-0 overflow-auto px-4 py-4"
  >
    <!-- Bottom-anchored stack: a min-height flex column with `mt-auto` on the
         content wrapper sinks a short thread to the bottom of the viewport, while
         a long thread overflows downward and scrolls normally from the top (a
         plain `justify-end` would push the overflow past the top edge and clip
         the oldest messages out of reach). -->
    <div class="flex min-h-full flex-col">
      <div class="mt-auto flex flex-col gap-4">
        {#if sessionStore.contextSummarized}
          <!-- Automatic context management: older turns were folded into a running
               summary to fit the model's window. Surfaced so the compaction is
               visible (it's otherwise a hidden system message). -->
          <div class="flex items-center gap-2 py-1 text-xs text-muted-foreground">
            <span class="h-px flex-1 bg-border"></span>
            <span class="shrink-0">Earlier conversation summarized to fit context</span>
            <span class="h-px flex-1 bg-border"></span>
          </div>
        {/if}
        {#if sessionStore.loadingOlder}
          <div class="flex justify-center py-1 text-xs text-muted-foreground">
            loading older…
          </div>
        {/if}
        {#each sessionStore.convo as entry (entry.id)}
          <Message {entry} isLast={entry.id === lastId} />
          {#if entry.stopped}
            <div class="flex items-center gap-1.5 -mt-2 text-xs text-muted-foreground">
              <Square class="size-3 fill-current" />
              <span>Stopped</span>
            </div>
          {/if}
        {/each}
        <!-- Dots are a pre-content placeholder ONLY: shown while busy until the
             live assistant turn has any segment. Once it does, the hoisted
             ThinkingBlock takes over (so the two never show at once). -->
        {#if sessionStore.busy && !(sessionStore.convo.at(-1)?.role === "assistant" && (sessionStore.convo.at(-1)?.segments.length ?? 0) > 0)}
          <div class="flex items-center gap-1.5 text-sm text-muted-foreground">
            <span class="inline-flex gap-1">
              <span class="size-1.5 animate-bounce rounded-full bg-current [animation-delay:-0.3s]"></span>
              <span class="size-1.5 animate-bounce rounded-full bg-current [animation-delay:-0.15s]"></span>
              <span class="size-1.5 animate-bounce rounded-full bg-current"></span>
            </span>
            <span>thinking</span>
          </div>
        {/if}
        {#each sessionStore.queue as q, i (i)}
          <div class="flex justify-end">
            <div
              class="flex max-w-[85%] items-center gap-2 rounded-2xl rounded-br-sm border border-dashed border-border bg-muted/50 px-4 py-2.5 text-sm leading-relaxed text-muted-foreground"
            >
              <span class="whitespace-pre-wrap">{q}</span>
              <button
                type="button"
                onclick={() => removeQueued(i)}
                aria-label="Remove queued message"
                class="shrink-0 rounded hover:text-foreground"
              >
                <X class="size-3.5" />
              </button>
            </div>
          </div>
        {/each}
        {#if sessionStore.convo.length === 0 && !sessionStore.busy}
          <div class="mx-auto flex max-w-sm flex-col items-center gap-3 py-12 text-center">
            <span class="flex size-11 items-center justify-center rounded-lg border border-border bg-card text-muted-foreground">
              <MessagesSquare class="size-5" />
            </span>
            <div class="flex flex-col gap-1">
              <p class="font-display text-base font-medium text-foreground">Start a conversation</p>
              <p class="text-sm text-muted-foreground">
                Ask zanto a question or type <span class="font-mono">/</span> for commands and
                <span class="font-mono">@</span> to attach a file.
              </p>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>

  {#if !atBottom}
    <button
      type="button"
      onclick={scrollToBottom}
      aria-label="Jump to latest"
      class="absolute bottom-4 left-1/2 z-10 flex -translate-x-1/2 items-center gap-1.5 rounded-full border border-border bg-background/90 px-3 py-1.5 text-xs font-medium text-muted-foreground shadow-md backdrop-blur transition-colors hover:text-foreground"
    >
      <ArrowDown class="size-3.5" />
      Jump to latest
    </button>
  {/if}
</div>
