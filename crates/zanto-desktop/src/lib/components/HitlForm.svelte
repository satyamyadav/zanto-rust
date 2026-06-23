<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Button } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import * as Select from "$lib/components/ui/select";
  import { toast } from "svelte-sonner";
  import { ipc, type InteractionRequest } from "$lib/ipc";
  import CopyIcon from "@lucide/svelte/icons/copy";

  // The single HITL surface above the composer: permission approvals and agent forms.
  let req = $state<InteractionRequest | null>(null);
  let stepIdx = $state(0);
  let answers = $state<Record<string, any>>({});

  let panel = $state<HTMLDivElement | null>(null);
  // Element that had focus before the overlay opened, so we can restore it on close.
  let returnFocus: HTMLElement | null = null;

  onMount(() => {
    const un = ipc.onInteractionRequest(async (r) => {
      returnFocus = document.activeElement as HTMLElement | null;
      req = r;
      stepIdx = 0;
      answers = {};
      await tick();
      focusFirst();
    });
    return () => un.then((f) => f());
  });

  function focusFirst() {
    const target = panel?.querySelector<HTMLElement>(
      "input, select, textarea, button, [tabindex]:not([tabindex='-1'])"
    );
    target?.focus();
  }

  function close(r: InteractionRequest | null, value: unknown) {
    req = null;
    if (r) ipc.respond(r.id, value);
    // Restore focus to wherever it was before the overlay grabbed it.
    returnFocus?.focus();
    returnFocus = null;
  }

  function approve(value: "once" | "session" | "forever" | "deny") {
    close(req, value);
  }

  function submitForm() {
    close(req, answers);
  }

  async function copyPath(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      toast.success("Path copied");
    } catch (e) {
      toast.error("Could not copy the path", { description: `${e}` });
    }
  }

  // Trap Tab within the panel and let Esc dismiss (deny approval / cancel form).
  function onKeydown(e: KeyboardEvent) {
    if (!req || !panel) return;
    // An open Select (rendered in a portal) consumes Escape via preventDefault to
    // close its own dropdown — don't also tear down the whole request in that case.
    if (e.key === "Escape") {
      if (e.defaultPrevented) return;
      e.preventDefault();
      close(req, req.kind === "approval" ? "deny" : null);
      return;
    }
    // Enter in a text field: advance step or submit.
    // IMPORTANT: check e.target (where the event ORIGINATED), not document.activeElement.
    // By the time the event bubbles to the window, focus may have moved elsewhere
    // (e.g. Svelte/focusFirst() just focused a HITL input synchronously after the
    // interaction_request event fired — so activeElement is the HITL input even though
    // the keydown originated in the composer textarea). Using e.target is the correct
    // source-of-truth: it never changes as the event bubbles.
    // Guard: only when Enter originated from an <input> inside this panel (text fields).
    // Skip Shift+Enter (multiline intent). Select trigger buttons are <button> elements,
    // not <input>, so they are naturally excluded — pressing Enter on a closed Select
    // trigger opens the dropdown instead. When a Select listbox is open, the listbox
    // is portaled outside the panel, so e.target would be inside the listbox (not an
    // <input>) and panel.contains(e.target) is false.
    if (e.key === "Enter" && !e.shiftKey && req.kind === "form") {
      const target = e.target as Element;
      // Exclude checkboxes (confirm fields): Enter there should toggle natively, not advance.
      if (
        target instanceof HTMLInputElement &&
        target.type !== "checkbox" &&
        panel.contains(target)
      ) {
        e.preventDefault();
        if (isLast) {
          submitForm();
        } else {
          stepIdx += 1;
        }
        return;
      }
    }
    if (e.key !== "Tab") return;
    // When focus is inside a portaled Select listbox (outside `panel`), let the
    // Select own its keyboard nav rather than fighting it from here.
    if (!panel.contains(document.activeElement)) return;
    const focusables = Array.from(
      panel.querySelectorAll<HTMLElement>(
        "input:not([disabled]), select:not([disabled]), textarea:not([disabled]), button:not([disabled]), [tabindex]:not([tabindex='-1'])"
      )
    ).filter((el) => el.offsetParent !== null || el === document.activeElement);
    if (focusables.length === 0) return;
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    const active = document.activeElement as HTMLElement;
    if (e.shiftKey && active === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && active === last) {
      e.preventDefault();
      first.focus();
    }
  }

  const steps = $derived(req?.steps ?? []);
  const isLast = $derived(stepIdx >= steps.length - 1);
  const stepFields = $derived(steps[stepIdx]?.fields ?? []);

  // Seed each select to its first option so a required select the user never
  // opens still submits a value — mirrors the old native <select> default.
  $effect(() => {
    for (const f of stepFields) {
      if (f.type === "select" && answers[f.name] == null && f.options?.length) {
        answers[f.name] = f.options[0];
      }
    }
  });
</script>

<svelte:window onkeydown={onKeydown} />

{#if req}
  <div
    bind:this={panel}
    role="dialog"
    aria-modal="true"
    aria-label={req.kind === "approval" ? "Permission request" : (req.title ?? "Agent form")}
    class="absolute bottom-full left-0 right-0 mb-2 mx-3 rounded-lg border border-border bg-popover text-popover-foreground shadow-lg p-3 z-20"
  >
    {#if req.kind === "approval"}
      <div class="mb-1 flex items-center gap-2 text-sm">
        <span class="font-display text-xs font-semibold uppercase tracking-wide text-muted-foreground">{req.op}</span>
        <span class="font-mono">"{req.path}"</span>
      </div>
      <div class="mb-2 flex items-start gap-1.5">
        <code class="flex-1 break-all font-mono text-xs text-muted-foreground select-text">{req.resolved}</code>
        {#if req.resolved}
          <Button
            size="icon-xs"
            variant="ghost"
            class="shrink-0"
            aria-label="Copy path"
            onclick={() => copyPath(req!.resolved!)}
          >
            <CopyIcon class="size-3" />
          </Button>
        {/if}
      </div>
      <div class="flex gap-2">
        <Button size="sm" onclick={() => approve("once")}>Allow once</Button>
        <Button size="sm" variant="secondary" onclick={() => approve("session")}>Allow this session</Button>
        <Button size="sm" variant="secondary" onclick={() => approve("forever")}>Always allow</Button>
        <Button size="sm" variant="destructive" class="ml-auto" onclick={() => approve("deny")}>Deny</Button>
      </div>
    {:else}
      <div class="mb-2 flex items-center justify-between gap-2">
        {#if req.title}<div class="font-display text-sm font-semibold">{req.title}</div>{/if}
        {#if steps.length > 1}
          <span class="shrink-0 rounded-full bg-muted px-2 py-0.5 font-mono text-xs text-muted-foreground">
            Step {stepIdx + 1} of {steps.length}
          </span>
        {/if}
      </div>
      {#each stepFields as f (f.name)}
        <div class="mb-2 space-y-1">
          <label class="text-xs text-muted-foreground" for={`hitl-${f.name}`}>
            {f.label}
            {#if f.type !== "confirm"}<span class="text-destructive" aria-hidden="true">*</span>{/if}
          </label>
          {#if f.type === "select"}
            <Select.Root type="single" bind:value={answers[f.name]}>
              <Select.Trigger
                id={`hitl-${f.name}`}
                class="w-full focus-visible:ring-2 focus-visible:ring-ring"
              >
                {answers[f.name] ?? "Choose an option"}
              </Select.Trigger>
              <Select.Content>
                {#each f.options ?? [] as o (o)}
                  <Select.Item value={o} label={o} />
                {/each}
              </Select.Content>
            </Select.Root>
          {:else if f.type === "confirm"}
            <label class="flex items-center gap-2 text-sm">
              <input
                id={`hitl-${f.name}`}
                type="checkbox"
                class="size-4 accent-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
                bind:checked={answers[f.name]}
              />
              <span class="text-muted-foreground">Confirm</span>
            </label>
          {:else}
            <Input
              id={`hitl-${f.name}`}
              class="focus-visible:ring-2 focus-visible:ring-ring"
              bind:value={answers[f.name]}
            />
          {/if}
        </div>
      {/each}
      <div class="flex items-center gap-2 pt-1">
        <Button size="sm" variant="ghost" onclick={() => close(req, null)}>Cancel</Button>
        <div class="ml-auto flex gap-2">
          {#if stepIdx > 0}
            <Button size="sm" variant="secondary" onclick={() => (stepIdx -= 1)}>Back</Button>
          {/if}
          {#if isLast}
            <Button size="sm" onclick={submitForm}>Submit</Button>
          {:else}
            <Button size="sm" onclick={() => (stepIdx += 1)}>Next</Button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
{/if}
