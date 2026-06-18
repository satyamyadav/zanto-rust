<script lang="ts">
  type ChartData = {
    type: "bar" | "line" | "pie" | "doughnut";
    title?: string;
    labels: string[];
    datasets: { label?: string; data: number[] }[];
  };

  let { data }: { data: ChartData } = $props();

  // Series palette expressed purely as CSS-variable expressions so the webview
  // resolves them at paint time — no getComputedStyle, no JS timing race. The
  // four genuinely-hued status tokens lead; color-mix variants extend past four
  // series while staying on-brand and dark-mode safe. ≥6 distinct entries.
  const PALETTE = [
    "var(--primary)",
    "var(--success)",
    "var(--destructive)",
    "var(--warning)",
    "color-mix(in oklch, var(--primary) 60%, var(--success))",
    "color-mix(in oklch, var(--warning) 60%, var(--destructive))",
  ];
  const color = (i: number) => PALETTE[((i % PALETTE.length) + PALETTE.length) % PALETTE.length];

  const AXIS = "var(--muted-foreground)";
  const GRID = "var(--border)";

  // Drawing surface in viewBox units. The <svg> scales to the container via
  // width=100% + viewBox, so these are layout-relative, not pixels.
  const W = 600;
  const H = 280;
  const PAD = { top: 16, right: 16, bottom: 36, left: 44 };

  // --- normalised inputs (guard malformed artifacts) ---
  const chartType = $derived(data?.type ?? "bar");
  const title = $derived(data?.title);
  const labels = $derived(data?.labels ?? []);
  const datasets = $derived(
    (data?.datasets ?? []).map((ds) => ({
      label: ds?.label,
      data: ds?.data ?? [],
    })),
  );
  const isArc = $derived(chartType === "pie" || chartType === "doughnut");
  const showLegend = $derived(isArc || datasets.length > 1);

  const fmt = (n: number) => {
    if (!Number.isFinite(n)) return "";
    if (Number.isInteger(n)) return n.toLocaleString();
    if (Math.abs(n) >= 1000) return Math.round(n).toLocaleString();
    // Up to 2 decimals, trailing zeros stripped; avoids rounding a sub-1000
    // value up across the thousands boundary and never collapses a small
    // non-zero value to a bare "0".
    const r = Number(n.toFixed(2));
    return Number.isInteger(r) ? r.toLocaleString() : String(r);
  };

  // --- cartesian (bar/line) geometry ---
  const plot = {
    x: PAD.left,
    y: PAD.top,
    w: W - PAD.left - PAD.right,
    h: H - PAD.top - PAD.bottom,
  };

  // Max across all series, never below 1 so an empty/zero chart doesn't divide
  // by zero. Negative values clamp to 0 (axis starts at zero).
  const maxVal = $derived(
    Math.max(1, ...datasets.flatMap((ds) => ds.data.map((v) => (Number.isFinite(v) ? v : 0))), 0),
  );

  const valueToY = (v: number) =>
    plot.y + plot.h * (1 - Math.max(0, Number.isFinite(v) ? v : 0) / maxVal);

  // Y gridlines / ticks: 4 intervals.
  const yTicks = $derived(
    Array.from({ length: 5 }, (_, i) => {
      const t = i / 4;
      return { value: maxVal * t, y: plot.y + plot.h * (1 - t) };
    }),
  );

  // Per-label horizontal band; bars share the band, grouped by dataset.
  const bandW = $derived(labels.length > 0 ? plot.w / labels.length : plot.w);
  const bandX = (li: number) => plot.x + bandW * li;
  const bandMid = (li: number) => bandX(li) + bandW / 2;

  // Bars (grouped). Inner padding 18% of band; remaining split across datasets.
  const barInnerW = $derived(bandW * 0.82);
  const barGroupPad = $derived(bandW * 0.09);
  const barW = $derived(datasets.length > 0 ? barInnerW / datasets.length : barInnerW);
  const barX = (li: number, di: number) => bandX(li) + barGroupPad + barW * di;

  type Bar = { x: number; y: number; w: number; h: number; fill: string };
  const bars = $derived.by<Bar[]>(() => {
    if (chartType !== "bar") return [];
    const out: Bar[] = [];
    labels.forEach((_, li) => {
      datasets.forEach((ds, di) => {
        const v = ds.data[li];
        if (v == null || !Number.isFinite(v)) return;
        const y = valueToY(v);
        out.push({
          x: barX(li, di),
          y,
          w: Math.max(0, barW - 1),
          h: Math.max(0, plot.y + plot.h - y),
          fill: color(di),
        });
      });
    });
    return out;
  });

  // Line points centered in each band.
  type Pt = { x: number; y: number };
  type Line = { stroke: string; points: Pt[]; poly: string };
  const lines = $derived.by<Line[]>(() => {
    if (chartType !== "line") return [];
    return datasets.map((ds, di) => {
      const points: Pt[] = [];
      labels.forEach((_, li) => {
        const v = ds.data[li];
        if (v == null || !Number.isFinite(v)) return;
        points.push({ x: bandMid(li), y: valueToY(v) });
      });
      return {
        stroke: color(di),
        points,
        poly: points.map((p) => `${p.x},${p.y}`).join(" "),
      };
    });
  });

  // --- arc (pie/doughnut) geometry ---
  const cx = W / 2;
  const cy = H / 2;
  const radius = Math.min(W, H) / 2 - 24;
  const innerR = $derived(chartType === "doughnut" ? radius * 0.58 : 0);

  type Slice = { d: string; fill: string };
  const slices = $derived.by<Slice[]>(() => {
    if (!isArc) return [];
    const vals = (datasets[0]?.data ?? []).map((v) => (Number.isFinite(v) && v > 0 ? v : 0));
    const total = vals.reduce((a, b) => a + b, 0);
    if (total <= 0) return [];
    const out: Slice[] = [];
    let acc = 0;
    vals.forEach((v, i) => {
      if (v <= 0) return;
      const a0 = (acc / total) * Math.PI * 2 - Math.PI / 2;
      acc += v;
      const a1 = (acc / total) * Math.PI * 2 - Math.PI / 2;
      const large = a1 - a0 > Math.PI ? 1 : 0;
      // A single full-circle slice can't be drawn with one arc (start==end);
      // split it into two half-arcs so the path closes.
      const drawArc = (s: number, e: number, lg: number): string => {
        const x0 = cx + radius * Math.cos(s);
        const y0 = cy + radius * Math.sin(s);
        const x1 = cx + radius * Math.cos(e);
        const y1 = cy + radius * Math.sin(e);
        if (innerR > 0) {
          const ix0 = cx + innerR * Math.cos(s);
          const iy0 = cy + innerR * Math.sin(s);
          const ix1 = cx + innerR * Math.cos(e);
          const iy1 = cy + innerR * Math.sin(e);
          return (
            `M ${x0} ${y0} A ${radius} ${radius} 0 ${lg} 1 ${x1} ${y1} ` +
            `L ${ix1} ${iy1} A ${innerR} ${innerR} 0 ${lg} 0 ${ix0} ${iy0} Z`
          );
        }
        return `M ${cx} ${cy} L ${x0} ${y0} A ${radius} ${radius} 0 ${lg} 1 ${x1} ${y1} Z`;
      };
      let d: string;
      if (vals.filter((x) => x > 0).length === 1) {
        const mid = a0 + Math.PI;
        d = `${drawArc(a0, mid, 1)} ${drawArc(mid, a1, 1)}`;
      } else {
        d = drawArc(a0, a1, large);
      }
      out.push({ d, fill: color(i) });
    });
    return out;
  });

  // Legend entries: per-dataset for bar/line, per-label for arc.
  type LegendItem = { key: string; label: string; color: string };
  const legend = $derived.by<LegendItem[]>(() => {
    if (isArc) {
      return labels.map((l, i) => ({ key: `${i}`, label: l ?? "", color: color(i) }));
    }
    return datasets.map((ds, i) => ({
      key: `${i}`,
      label: ds.label ?? `Series ${i + 1}`,
      color: color(i),
    }));
  });

  const hasData = $derived(
    isArc ? slices.length > 0 : labels.length > 0 && datasets.length > 0,
  );
</script>

<div>
  {#if title}
    <div class="mb-2 text-xs font-medium text-muted-foreground">{title}</div>
  {/if}

  {#if showLegend && legend.length > 0}
    <div class="mb-2 flex flex-wrap gap-x-3 gap-y-1 text-xs text-muted-foreground">
      {#each legend as item (item.key)}
        <span class="inline-flex items-center gap-1.5">
          <span
            class="inline-block h-2.5 w-2.5 rounded-[2px]"
            style="background-color: {item.color}"
          ></span>
          {item.label}
        </span>
      {/each}
    </div>
  {/if}

  <div class="h-64 w-full text-muted-foreground">
    {#if !hasData}
      <div class="flex h-full items-center justify-center text-xs text-muted-foreground">
        No data
      </div>
    {:else}
      <svg
        viewBox="0 0 {W} {H}"
        width="100%"
        height="100%"
        preserveAspectRatio="xMidYMid meet"
        role="img"
        aria-label={title ?? "chart"}
      >
        {#if isArc}
          {#each slices as s, i (i)}
            <path d={s.d} fill={s.fill} stroke="var(--card)" stroke-width="1" />
          {/each}
        {:else}
          <!-- gridlines + y ticks -->
          {#each yTicks as t (t.y)}
            <line
              x1={plot.x}
              y1={t.y}
              x2={plot.x + plot.w}
              y2={t.y}
              stroke={GRID}
              stroke-width="1"
            />
            <text x={plot.x - 6} y={t.y + 3} text-anchor="end" font-size="10" fill={AXIS}
              >{fmt(t.value)}</text
            >
          {/each}

          <!-- baseline axis -->
          <line
            x1={plot.x}
            y1={plot.y + plot.h}
            x2={plot.x + plot.w}
            y2={plot.y + plot.h}
            stroke={GRID}
            stroke-width="1"
          />

          <!-- x labels -->
          {#each labels as label, li (li)}
            <text
              x={bandMid(li)}
              y={plot.y + plot.h + 16}
              text-anchor="middle"
              font-size="10"
              fill={AXIS}>{label}</text
            >
          {/each}

          {#if chartType === "bar"}
            {#each bars as b, i (i)}
              <rect x={b.x} y={b.y} width={b.w} height={b.h} fill={b.fill} rx="1" />
            {/each}
          {:else}
            {#each lines as ln, i (i)}
              {#if ln.points.length > 1}
                <polyline
                  points={ln.poly}
                  fill="none"
                  stroke={ln.stroke}
                  stroke-width="2"
                  stroke-linejoin="round"
                  stroke-linecap="round"
                />
              {/if}
              {#each ln.points as p, pi (pi)}
                <circle cx={p.x} cy={p.y} r="2.5" fill={ln.stroke} />
              {/each}
            {/each}
          {/if}
        {/if}
      </svg>
    {/if}
  </div>
</div>
