<script lang="ts">
  import { onDestroy } from "svelte";
  import {
    Chart,
    BarController,
    BarElement,
    LineController,
    LineElement,
    PointElement,
    PieController,
    DoughnutController,
    ArcElement,
    CategoryScale,
    LinearScale,
    Tooltip,
    Legend,
    type ChartConfiguration,
    type ChartType,
  } from "chart.js";

  Chart.register(
    BarController,
    BarElement,
    LineController,
    LineElement,
    PointElement,
    PieController,
    DoughnutController,
    ArcElement,
    CategoryScale,
    LinearScale,
    Tooltip,
    Legend,
  );

  type ChartData = {
    type: "bar" | "line" | "pie" | "doughnut";
    title?: string;
    labels: string[];
    datasets: { label?: string; data: number[] }[];
  };

  let { data }: { data: ChartData } = $props();

  // Categorical palette — works in both themes.
  const PALETTE = [
    "oklch(0.62 0.19 278)",
    "oklch(0.68 0.17 162)",
    "oklch(0.72 0.16 70)",
    "oklch(0.64 0.21 27)",
    "oklch(0.66 0.16 320)",
    "oklch(0.6 0.15 220)",
    "oklch(0.7 0.15 130)",
    "oklch(0.62 0.18 350)",
  ];

  let canvas: HTMLCanvasElement | undefined = $state();
  let chart: Chart | undefined;

  function readVar(name: string, fallback: string): string {
    if (typeof window === "undefined" || !canvas) return fallback;
    const v = getComputedStyle(canvas).getPropertyValue(name).trim();
    return v || fallback;
  }

  function build(el: HTMLCanvasElement, d: ChartData) {
    const arc = d.type === "pie" || d.type === "doughnut";
    const fg = readVar("--muted-foreground", "#888");
    const grid = readVar("--border", "rgba(128,128,128,0.2)");

    const datasets = d.datasets.map((ds, i) => {
      if (arc) {
        return {
          label: ds.label,
          data: ds.data,
          backgroundColor: d.labels.map((_, j) => PALETTE[j % PALETTE.length]),
          borderColor: readVar("--background", "#fff"),
          borderWidth: 1,
        };
      }
      const c = PALETTE[i % PALETTE.length];
      return {
        label: ds.label,
        data: ds.data,
        backgroundColor: c,
        borderColor: c,
        borderWidth: 2,
        tension: 0.25,
      };
    });

    const config: ChartConfiguration = {
      type: d.type as ChartType,
      data: { labels: d.labels, datasets },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: {
            display: arc || d.datasets.length > 1,
            labels: { color: fg },
          },
        },
        scales: arc
          ? {}
          : {
              x: { ticks: { color: fg }, grid: { color: grid } },
              y: { ticks: { color: fg }, grid: { color: grid } },
            },
      },
    };
    return new Chart(el, config);
  }

  $effect(() => {
    if (!canvas) return;
    chart = build(canvas, data);
    return () => {
      chart?.destroy();
      chart = undefined;
    };
  });

  onDestroy(() => {
    chart?.destroy();
    chart = undefined;
  });
</script>

<div>
  {#if data.title}
    <div class="text-xs text-muted-foreground mb-2">{data.title}</div>
  {/if}
  <div class="relative h-64 w-full">
    <canvas bind:this={canvas}></canvas>
  </div>
</div>
