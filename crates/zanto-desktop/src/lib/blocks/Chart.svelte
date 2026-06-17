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

  let canvas: HTMLCanvasElement | undefined = $state();
  let chart: Chart | undefined;

  function readVar(name: string): string {
    if (!canvas) return "";
    return getComputedStyle(canvas).getPropertyValue(name).trim();
  }

  // Categorical palette derived entirely from CSS tokens — primary (signal-amber)
  // leads the series; the rest are the genuinely-hued status tokens so every
  // series stays visually distinct (the neutral foreground tokens are reserved
  // for axes/grid, never for series). On-brand and dark-mode safe, no hex.
  function palette(): string[] {
    const tokens = ["--primary", "--success", "--destructive", "--warning"];
    const base = tokens.map(readVar).filter((c) => c.length > 0);
    if (base.length === 0) return [];
    // Extend with token-mixed variants so >4 series stay separable without
    // introducing flat grays.
    const fg = readVar("--foreground");
    const variants = fg
      ? base.map((c) => `color-mix(in oklch, ${c} 65%, ${fg})`)
      : [];
    return [...base, ...variants];
  }

  function build(el: HTMLCanvasElement, d: ChartData) {
    const arc = d.type === "pie" || d.type === "doughnut";
    const fg = readVar("--muted-foreground");
    const grid = readVar("--border");
    const surface = readVar("--card");
    // Floor to a token color so pick() never indexes an empty array.
    const colors = palette();
    if (colors.length === 0) colors.push(readVar("--foreground") || "currentColor");
    const pick = (i: number) => colors[i % colors.length];

    const datasets = d.datasets.map((ds, i) => {
      if (arc) {
        return {
          label: ds.label,
          data: ds.data,
          backgroundColor: d.labels.map((_, j) => pick(j)),
          borderColor: surface,
          borderWidth: 1,
        };
      }
      const c = pick(i);
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
    <div class="mb-2 text-xs font-medium text-muted-foreground">{data.title}</div>
  {/if}
  <div class="relative h-64 w-full">
    <canvas bind:this={canvas}></canvas>
  </div>
</div>
