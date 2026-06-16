// Density store (Svelte 5 runes). Light/dark is handled by `mode-watcher`
// (which sonner reads) — see +layout.svelte's <ModeWatcher/>. Density scales the
// whole rem-based layout via html[data-density].

export type Density = "compact" | "normal" | "relaxed";

const DENSITY_KEY = "zanto-density";

export const density = $state({ value: "normal" as Density });

export function bootstrapDensity() {
  const stored = localStorage.getItem(DENSITY_KEY);
  density.value =
    stored === "compact" || stored === "normal" || stored === "relaxed" ? stored : "normal";
  document.documentElement.dataset.density = density.value;
}

export function setDensity(value: Density) {
  density.value = value;
  localStorage.setItem(DENSITY_KEY, value);
  document.documentElement.dataset.density = value;
}
