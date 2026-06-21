<script lang="ts">
  import * as Select from "$lib/components/ui/select";
  import { Input } from "$lib/components/ui/input";
  import { Textarea } from "$lib/components/ui/textarea";
  import type { GenerationParams } from "$lib/ipc";

  // The bound params object is mutated in place. Empty/unset fields are left
  // `undefined`; the parent strips them before sending (see cleanGeneration).
  let { params = $bindable() }: { params: GenerationParams } = $props();

  const DEFAULT = "__default__";
  const efforts = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];
  const choices = ["auto", "none", "required"];

  function isNumeric(s: string | undefined): boolean {
    return !!s && Number.isFinite(Number(s));
  }

  // Text/number mirrors for the structured fields; re-seed when the bound object
  // reference swaps (e.g. switching the active provider's override).
  let lastRef = params;
  let stopText = $state((params.stop_sequences ?? []).join("\n"));
  let bodyText = $state(params.extra_body != null ? JSON.stringify(params.extra_body, null, 2) : "");
  let bodyError = $state("");
  let budget = $state<number | "">(isNumeric(params.reasoning_effort) ? Number(params.reasoning_effort) : "");

  $effect(() => {
    if (params !== lastRef) {
      lastRef = params;
      stopText = (params.stop_sequences ?? []).join("\n");
      bodyText = params.extra_body != null ? JSON.stringify(params.extra_body, null, 2) : "";
      bodyError = "";
      budget = isNumeric(params.reasoning_effort) ? Number(params.reasoning_effort) : "";
    }
  });

  // A numeric reasoning budget (when set) takes precedence and is stored as the
  // numeric string; otherwise the effort keyword applies.
  const effortValue = $derived(isNumeric(params.reasoning_effort) ? DEFAULT : (params.reasoning_effort ?? DEFAULT));

  function setEffort(v: string) {
    params.reasoning_effort = v === DEFAULT ? undefined : v;
    if (v !== DEFAULT) budget = "";
  }
  function onBudget(v: string) {
    budget = v === "" ? "" : Number(v);
    params.reasoning_effort = v === "" ? undefined : String(v);
  }
  function setChoice(v: string) {
    params.tool_choice = v === DEFAULT ? undefined : v;
  }
  function onStop(v: string) {
    stopText = v;
    const arr = v.split("\n").map((s) => s.trim()).filter(Boolean);
    params.stop_sequences = arr.length ? arr : undefined;
  }
  function onBody(v: string) {
    bodyText = v;
    if (!v.trim()) {
      params.extra_body = undefined;
      bodyError = "";
      return;
    }
    try {
      params.extra_body = JSON.parse(v);
      bodyError = "";
    } catch {
      bodyError = "Invalid JSON — not saved until fixed.";
    }
  }
</script>

<div class="space-y-3">
  <div class="grid grid-cols-2 gap-3">
    <label class="space-y-1 text-xs text-muted-foreground">Temperature
      <Input type="number" step="0.1" min="0" bind:value={params.temperature} class="font-mono" />
    </label>
    <label class="space-y-1 text-xs text-muted-foreground">Max tokens
      <Input type="number" step="1" min="1" bind:value={params.max_tokens} class="font-mono" />
    </label>
    <label class="space-y-1 text-xs text-muted-foreground">Top-p
      <Input type="number" step="0.05" min="0" max="1" bind:value={params.top_p} class="font-mono" />
    </label>
    <label class="space-y-1 text-xs text-muted-foreground">Seed
      <Input type="number" step="1" bind:value={params.seed} class="font-mono" />
    </label>
  </div>

  <div class="grid grid-cols-2 gap-3">
    <div class="space-y-1">
      <span class="text-xs text-muted-foreground">Reasoning effort</span>
      <Select.Root type="single" value={effortValue} onValueChange={setEffort}>
        <Select.Trigger class="w-full">{effortValue === DEFAULT ? "default" : effortValue}</Select.Trigger>
        <Select.Content>
          <Select.Item value={DEFAULT} label="default" />
          {#each efforts as e (e)}<Select.Item value={e} label={e} />{/each}
        </Select.Content>
      </Select.Root>
    </div>
    <label class="space-y-1 text-xs text-muted-foreground">Reasoning budget (tokens)
      <Input
        type="number"
        step="1"
        min="1"
        value={budget}
        oninput={(e) => onBudget((e.target as HTMLInputElement).value)}
        placeholder="overrides effort"
        class="font-mono"
      />
    </label>
  </div>

  <div class="grid grid-cols-2 items-center gap-3">
    <div class="space-y-1">
      <span class="text-xs text-muted-foreground">Tool choice</span>
      <Select.Root type="single" value={params.tool_choice ?? DEFAULT} onValueChange={setChoice}>
        <Select.Trigger class="w-full">{params.tool_choice ?? "default"}</Select.Trigger>
        <Select.Content>
          <Select.Item value={DEFAULT} label="default" />
          {#each choices as c (c)}<Select.Item value={c} label={c} />{/each}
        </Select.Content>
      </Select.Root>
    </div>
    <label class="flex items-center gap-2 self-end pb-2 text-xs text-muted-foreground">
      <input
        type="checkbox"
        checked={params.json_mode === true}
        onchange={(e) => (params.json_mode = (e.target as HTMLInputElement).checked || undefined)}
        class="size-4 rounded border-border accent-primary"
      />
      Force JSON output
    </label>
  </div>

  <label class="space-y-1 text-xs text-muted-foreground">Stop sequences (one per line)
    <Textarea
      value={stopText}
      oninput={(e) => onStop((e.target as HTMLTextAreaElement).value)}
      rows={2}
      class="font-mono text-xs"
    />
  </label>

  <label class="space-y-1 text-xs text-muted-foreground">Advanced: extra_body (JSON)
    <Textarea
      value={bodyText}
      oninput={(e) => onBody((e.target as HTMLTextAreaElement).value)}
      rows={3}
      placeholder={'{ "key": "value" }'}
      class="font-mono text-xs"
    />
  </label>
  {#if bodyError}
    <p class="text-xs text-destructive">{bodyError}</p>
  {/if}
</div>
