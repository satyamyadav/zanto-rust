<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  // Type-only import — erased at compile time, so it never references `window`
  // at build/prerender. The runtime import stays dynamic inside onMount.
  import type ApexChartsType from "apexcharts";

  type ChartData = {
    type: "bar" | "line" | "pie" | "doughnut";
    title?: string;
    labels: string[];
    datasets: { label?: string; data: number[] }[];
  };

  let { data }: { data: ChartData } = $props();

  // Fixed hex palette (violet-led, brand-aligned) — deliberately NOT oklch CSS
  // vars, so SVG fills are safe across webview color-space support.
  const SERIES = ["#7c3aed", "#0ea5e9", "#22c55e", "#f59e0b", "#ef4444", "#a855f7"];
  const LIGHT = { fore: "#52525b", grid: "#e4e4e7" };
  const DARK = { fore: "#a1a1aa", grid: "#27272a" };

  let el: HTMLDivElement;
  let chart: ApexChartsType | null = null;

  function buildOptions(d: ChartData) {
    const dark = typeof document !== "undefined" && document.documentElement.classList.contains("dark");
    const c = dark ? DARK : LIGHT;
    const isArc = d.type === "pie" || d.type === "doughnut";
    const apexType = d.type === "doughnut" ? "donut" : d.type;
    const labels = d.labels ?? [];
    const datasets = d.datasets ?? [];

    const base: Record<string, unknown> = {
      chart: { type: apexType, height: 256, background: "transparent", toolbar: { show: false }, fontFamily: "inherit", foreColor: c.fore },
      colors: SERIES,
      theme: { mode: dark ? "dark" : "light" },
      tooltip: { theme: dark ? "dark" : "light" },
      grid: { borderColor: c.grid },
      dataLabels: { enabled: false },
      // ApexCharts reads `cnf.title.text` unconditionally — passing `undefined`
      // for `title` throws "undefined is not an object". Always give a title
      // object with (possibly empty) text.
      title: { text: d.title ?? "", style: { fontSize: "12px", fontWeight: 500, color: c.fore } },
      noData: { text: "No data", style: { color: c.fore } },
    };

    if (isArc) {
      return {
        ...base,
        series: (datasets[0]?.data ?? []).map((n) => (Number.isFinite(n) ? n : 0)),
        labels,
        legend: { position: "bottom" as const, horizontalAlign: "center" as const },
      };
    }
    return {
      ...base,
      series: datasets.map((ds, i) => ({ name: ds.label ?? `Series ${i + 1}`, data: ds.data ?? [] })),
      xaxis: { categories: labels },
      yaxis: {
        labels: {
          formatter: (v: number) => {
            if (!Number.isFinite(v)) return "";
            const abs = Math.abs(v);
            if (abs >= 1000) return `${(v / 1000).toFixed(abs % 1000 === 0 ? 0 : 1)}k`;
            return Number.isInteger(v) ? String(v) : String(Math.round(v * 100) / 100);
          },
        },
      },
    };
  }

  onMount(async () => {
    try {
      const mod = await import("apexcharts");
      const ApexCharts = mod.default;
      chart = new ApexCharts(el, buildOptions(data));
      await chart.render();
    } catch (e) {
      // A bad config must not become an unhandled rejection that breaks the page.
      console.error("chart render failed", e);
    }
  });

  // Re-render in place when the artifact data changes (e.g. streaming update).
  // Guarded so a single bad update can't throw across the reactive boundary.
  $effect(() => {
    const opts = buildOptions(data);
    if (!chart) return;
    try {
      chart.updateOptions(opts, true, true);
    } catch (e) {
      console.error("chart update failed", e);
    }
  });

  onDestroy(() => chart?.destroy());
</script>

<div class="w-full" bind:this={el}></div>
