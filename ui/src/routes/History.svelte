<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type BreakCounts, type CategoryKind, type DailyUsage } from "../lib/api";
  import { addDays, categoryColor, duration, hoursShort, localDayLabel, percent } from "../lib/format";
  import { errorText } from "../lib/notify.svelte";
  import { usePageToolbar } from "../lib/page.svelte";
  import Card from "../lib/ui/Card.svelte";
  import Empty from "../lib/ui/Empty.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Segmented from "../lib/ui/Segmented.svelte";
  import StatTile from "../lib/ui/StatTile.svelte";
  import Tooltip from "../lib/ui/Tooltip.svelte";

  type Range = 7 | 14 | 30 | 90;
  const RANGES: readonly { id: Range; label: string }[] = [
    { id: 7, label: "7 days" },
    { id: 14, label: "14 days" },
    { id: 30, label: "30 days" },
    { id: 90, label: "90 days" },
  ];
  const KINDS: readonly CategoryKind[] = ["Work", "Game", "Web", "Other"];
  const HOUR = 3_600_000;
  const REFRESH_MS = 60_000;
  const RANGE_KEY = "ns.history.range";

  function storedRange(): Range {
    try {
      const n = Number(localStorage.getItem(RANGE_KEY));
      return RANGES.find((r) => r.id === n)?.id ?? 14;
    } catch {
      return 14;
    }
  }

  let range = $state<Range>(storedRange());
  let data = $state<{
    today: string;
    from: string;
    to: string;
    rows: DailyUsage[];
    breaks: BreakCounts | null;
  } | null>(null);
  let error = $state<string | null>(null);
  let seq = 0;
  let timer: ReturnType<typeof setInterval> | undefined;

  const stale = $derived(data !== null && dayCount(data.from, data.to) !== range);

  function dayCount(from: string, to: string): number {
    return Math.round((Date.parse(`${to}T12:00:00Z`) - Date.parse(`${from}T12:00:00Z`)) / 86_400_000) + 1;
  }

  async function load() {
    const r = range;
    const my = ++seq;
    try {
      const { today } = await api.state();
      const from = addDays(today, -(r - 1));
      const [rows, breaks] = await Promise.all([
        api.daily(from, today),
        // An older service has no /stats/breaks; the page works without it.
        api.breaks(from, today).then((b) => b.total).catch(() => null),
      ]);
      if (my !== seq) return;
      data = { today, from, to: today, rows, breaks };
      error = null;
    } catch (e) {
      if (my === seq) error = errorText(e);
    }
  }

  $effect(() => {
    void range;
    load();
  });

  onMount(() => (timer = setInterval(load, REFRESH_MS)));
  onDestroy(() => clearInterval(timer));

  function setRange(r: Range) {
    range = r;
    hover = null;
    try {
      localStorage.setItem(RANGE_KEY, String(r));
    } catch {
      // Not persisted; fine.
    }
  }

  // ---- Data shaping ----

  interface CatTotal {
    name: string;
    kind: CategoryKind;
    color: string;
    ms: number;
  }

  interface Day {
    day: string;
    total: number;
    byKind: Record<CategoryKind, number>;
    cats: CatTotal[];
    weekend: boolean;
    date: Date;
  }

  const days = $derived.by((): Day[] => {
    if (!data) return [];
    const n = dayCount(data.from, data.to);
    const map = new Map<string, Day>();
    const out: Day[] = [];
    for (let i = 0; i < n; i++) {
      const day = addDays(data.from, i);
      const date = new Date(`${day}T12:00:00Z`);
      const wd = date.getUTCDay();
      const d: Day = {
        day,
        total: 0,
        byKind: { Work: 0, Game: 0, Web: 0, Other: 0 },
        cats: [],
        weekend: wd === 0 || wd === 6,
        date,
      };
      map.set(day, d);
      out.push(d);
    }
    for (const r of data.rows) {
      const d = map.get(r.day);
      if (!d) continue;
      d.total += r.ms;
      d.byKind[r.kind] += r.ms;
      d.cats.push({ name: r.category, kind: r.kind, color: r.color ?? categoryColor[r.kind], ms: r.ms });
    }
    for (const d of out) d.cats.sort((a, b) => b.ms - a.ms);
    return out;
  });

  const total = $derived(days.reduce((s, d) => s + d.total, 0));
  const activeDays = $derived(days.filter((d) => d.total > 0).length);
  const longest = $derived(
    days.reduce<Day | null>((best, d) => (d.total > 0 && (!best || d.total > best.total) ? d : best), null),
  );

  const catTotals = $derived.by((): CatTotal[] => {
    const map = new Map<string, CatTotal>();
    for (const r of data?.rows ?? []) {
      const key = `${r.kind}\u0000${r.category}`;
      const c = map.get(key);
      if (c) c.ms += r.ms;
      else map.set(key, { name: r.category, kind: r.kind, color: r.color ?? categoryColor[r.kind], ms: r.ms });
    }
    return [...map.values()].filter((c) => c.ms > 0).sort((a, b) => b.ms - a.ms);
  });
  const maxCat = $derived(Math.max(1, ...catTotals.map((c) => c.ms)));

  const kindTotals = $derived(
    KINDS.map((k) => ({ kind: k, ms: days.reduce((s, d) => s + d.byKind[k], 0) })),
  );

  // ---- Chart geometry ----

  let width = $state(0);
  const W = $derived(Math.max(280, Math.round(width)));
  const H = $derived(W < 640 ? 220 : 260);
  const twoLine = $derived(days.length <= 14);
  const M = $derived({ l: 42, r: 8, t: 12, b: twoLine ? 36 : 24 });
  const plotW = $derived(W - M.l - M.r);
  const plotH = $derived(H - M.t - M.b);
  const band = $derived(days.length ? plotW / days.length : plotW);
  const barW = $derived(Math.max(2, Math.min(40, band * (days.length > 30 ? 0.7 : 0.6))));

  /** Y scale: 3-5 gridlines on a round hour step. */
  const scale = $derived.by(() => {
    const maxH = Math.max(...days.map((d) => d.total), 0) / HOUR;
    const steps = [0.25, 0.5, 1, 2, 3, 4, 6, 8];
    let step = steps[steps.length - 1];
    let n = Math.max(1, Math.ceil(maxH / step));
    for (const s of steps) {
      const k = Math.ceil(maxH / s);
      if (k <= 5) {
        step = s;
        n = k;
        break;
      }
    }
    n = Math.max(3, n);
    const top = n * step * HOUR;
    return { top, lines: Array.from({ length: n + 1 }, (_, i) => i * step * HOUR) };
  });

  const y = (ms: number) => M.t + plotH - (ms / scale.top) * plotH;

  /** Show every `labelEvery`-th x label so they fit; today is always labelled. */
  const labelEvery = $derived.by(() => {
    if (twoLine) return 1;
    return [1, 2, 3, 5, 7, 14].find((k) => band * k >= 20) ?? 30;
  });

  const narrowFmt = new Intl.DateTimeFormat("en-GB", { weekday: "narrow", timeZone: "UTC" });

  interface Piece {
    kind: CategoryKind;
    y: number;
    h: number;
    top: boolean;
  }

  function pieces(d: Day): Piece[] {
    const out: Piece[] = [];
    let acc = 0;
    const present = KINDS.filter((k) => d.byKind[k] > 0);
    present.forEach((k, i) => {
      const y0 = y(acc);
      acc += d.byKind[k];
      const y1 = y(acc);
      const isTop = i === present.length - 1;
      const gap = !isTop && y0 - y1 > 2 ? 1 : 0;
      out.push({ kind: k, y: y1 + gap, h: Math.max(0, y0 - y1 - gap), top: isTop });
    });
    return out;
  }

  /** A rect with rounded top corners. */
  function topRounded(x: number, yy: number, w: number, h: number): string {
    const r = Math.min(3, w / 2, h);
    return `M${x},${yy + h}V${yy + r}Q${x},${yy} ${x + r},${yy}H${x + w - r}Q${x + w},${yy} ${x + w},${yy + r}V${yy + h}Z`;
  }

  // ---- Interaction ----

  let hover = $state<{ i: number; x: number; y: number } | null>(null);
  const hovered = $derived(hover ? (days[hover.i] ?? null) : null);

  function openDay(day: string) {
    try {
      sessionStorage.setItem("ns.day", day);
    } catch {
      // The Today page then opens on today.
    }
    location.hash = "dashboard";
  }

  function onKey(e: KeyboardEvent, day: string) {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      openDay(day);
    }
  }

  function onFocus(e: FocusEvent, i: number) {
    const r = (e.currentTarget as Element).getBoundingClientRect();
    hover = { i, x: r.left + r.width / 2, y: r.top + r.height / 3 };
  }

  const dayTitle = (day: string, today: string) => {
    const label = localDayLabel(day, today);
    if (label === "Today" || label === "Yesterday") {
      const d = new Date(`${day}T12:00:00Z`);
      return `${label}, ${d.toLocaleDateString("en-GB", { weekday: "short", day: "numeric", month: "short", timeZone: "UTC" })}`;
    }
    return label;
  };

  $effect(() => usePageToolbar(tools));
</script>

{#snippet tools()}
  {#if error}
    <span class="err-icon"><Icon name="alert" size={16} title="Could not load: {error}" /></span>
  {/if}
  <Segmented options={RANGES} value={range} onchange={setRange} label="Range" />
{/snippet}

<div class="page" class:stale>
  {#if data === null}
    <div class="placeholder" style:height="330px"></div>
  {:else if total === 0}
    <Card>
      <Empty
        icon="history"
        title="No activity in the last {range} days"
        hint="Daily totals by category show up here once Nagsworth has tracked some time."
      />
    </Card>
  {:else}
    {@const today = data.today}
    <Card
      title="Active time per day"
      subtitle="{localDayLabel(data.from, today)} – {localDayLabel(data.to, today)} · click a day to open it"
    >
      <div class="chart" bind:clientWidth={width} style:height="{H}px">
        {#if width > 0}
          <svg
            width={W}
            height={H}
            viewBox="0 0 {W} {H}"
            role="group"
            aria-label="Active time per day by category"
            onpointerleave={() => (hover = null)}
          >
            <!-- Weekend columns -->
            {#each days as d, i (d.day)}
              {#if d.weekend}
                <rect class="weekend" x={M.l + i * band} y={M.t} width={band} height={plotH} />
              {/if}
            {/each}

            <!-- Hover column -->
            {#if hover}
              <rect class="hover-col" x={M.l + hover.i * band} y={M.t} width={band} height={plotH} rx="3" />
            {/if}

            <!-- Gridlines and y labels -->
            {#each scale.lines as v (v)}
              <line class="grid" class:base={v === 0} x1={M.l} x2={W - M.r} y1={y(v)} y2={y(v)} />
              <text class="ylabel" x={M.l - 8} y={y(v)} dy="0.32em">{v === 0 ? "0" : hoursShort(v)}</text>
            {/each}

            <!-- Bars -->
            {#each days as d, i (d.day)}
              {@const x = M.l + i * band + (band - barW) / 2}
              {#each pieces(d) as p (p.kind)}
                {#if p.top}
                  <path d={topRounded(x, p.y, barW, p.h)} style:fill={categoryColor[p.kind]} />
                {:else}
                  <rect {x} y={p.y} width={barW} height={p.h} style:fill={categoryColor[p.kind]} />
                {/if}
              {/each}
            {/each}

            <!-- X labels -->
            {#each days as d, i (d.day)}
              {@const cx = M.l + (i + 0.5) * band}
              {@const isToday = d.day === today}
              {#if twoLine}
                <text class="xlabel" class:today={isToday} x={cx} y={M.t + plotH + 15}>{narrowFmt.format(d.date)}</text>
                <text class="xlabel sub" class:today={isToday} x={cx} y={M.t + plotH + 29}>{d.date.getUTCDate()}</text>
              {:else if (days.length - 1 - i) % labelEvery === 0}
                <text class="xlabel" class:today={isToday} x={cx} y={M.t + plotH + 16}>{d.date.getUTCDate()}</text>
              {/if}
            {/each}

            <!-- Hit areas -->
            {#each days as d, i (d.day)}
              <rect
                class="hit"
                x={M.l + i * band}
                y={M.t}
                width={band}
                height={plotH + M.b}
                role="button"
                tabindex="0"
                aria-label="{dayTitle(d.day, today)}: {duration(d.total)}. Open in Today"
                onpointermove={(e) => (hover = { i, x: e.clientX, y: e.clientY })}
                onfocus={(e) => onFocus(e, i)}
                onblur={() => (hover = null)}
                onclick={() => openDay(d.day)}
                onkeydown={(e) => onKey(e, d.day)}
              />
            {/each}
          </svg>
        {/if}
      </div>

      <div class="legend small">
        {#each kindTotals as k (k.kind)}
          {#if k.ms > 0}
            <span class="legend-item">
              <span class="swatch" style:background={categoryColor[k.kind]}></span>
              {k.kind}
              <span class="muted num">{duration(k.ms)}</span>
            </span>
          {/if}
        {/each}
        <span class="legend-item">
          <span class="swatch weekend-key"></span>
          <span class="muted">Weekend</span>
        </span>
      </div>

    </Card>

    <div class="tiles">
      <StatTile label="Total active" value={duration(total)} sub="last {days.length} days" />
      <StatTile
        label="Average per day"
        value={duration(activeDays ? total / activeDays : 0)}
        sub="{activeDays} {activeDays === 1 ? 'day' : 'days'} with activity"
      />
      <StatTile
        label="Longest day"
        value={longest ? duration(longest.total) : "–"}
        sub={longest ? dayTitle(longest.day, today) : undefined}
      />
      <StatTile
        label="Top category"
        value={catTotals[0]?.name ?? "–"}
        sub={catTotals[0] ? `${duration(catTotals[0].ms)} · ${percent(catTotals[0].ms, total)}` : undefined}
        color={catTotals[0]?.color}
      />
    </div>

    {#if data.breaks}
      {@const b = data.breaks}
      <Card title="Breaks" subtitle="Breaks you started, and whether they ran to the end">
        {#if b.started === 0}
          <p class="small muted">
            No breaks started in this range. A break started from a nag or the tray shows up here.
          </p>
        {:else}
          <div class="tiles">
            <StatTile label="Breaks started" value={String(b.started)} sub="last {days.length} days" />
            <StatTile
              label="Ran to the end"
              value={String(b.completed)}
              sub="{percent(b.completed, b.started)} of started breaks"
            />
            <StatTile
              label="Ended early"
              value={String(b.ended_early)}
              sub={b.ended_early > 0 ? `${percent(b.ended_early, b.started)} of started breaks` : "none"}
            />
            <StatTile
              label="With exercises"
              value="{b.with_exercises_completed} of {b.with_exercises}"
              sub={b.with_exercises > 0 ? "routines run to the end" : "none suggested"}
            />
          </div>
        {/if}
      </Card>
    {/if}

    <Card title="Categories" subtitle="Totals for the range" padded={false}>
      <table class="table cats">
        <thead>
          <tr>
            <th>Category</th>
            <th class="barcol"><span class="sr-only">Share bar</span></th>
            <th class="num">Time</th>
            <th class="num">Share</th>
            <th class="num perday">Per day</th>
          </tr>
        </thead>
        <tbody>
          {#each catTotals as c (c.kind + c.name)}
            <tr>
              <td>
                <div class="cat">
                  <span class="swatch" style:background={c.color}></span>
                  <span class="truncate">{c.name}</span>
                  {#if c.name !== c.kind}<span class="tag">{c.kind}</span>{/if}
                </div>
              </td>
              <td class="barcol">
                <div class="bar"><span style:width="{(c.ms / maxCat) * 100}%" style:background={c.color}></span></div>
              </td>
              <td class="num">{duration(c.ms)}</td>
              <td class="num muted">{percent(c.ms, total)}</td>
              <td class="num muted perday">{duration(activeDays ? c.ms / activeDays : 0)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </Card>
  {/if}
</div>

<!-- Outside .page: its container query makes it the containing block for fixed elements. -->
<Tooltip x={hover?.x ?? 0} y={hover?.y ?? 0} show={hovered !== null}>
  {#if hovered}
    <div class="tt-head">{dayTitle(hovered.day, data?.today ?? hovered.day)}</div>
    {#if hovered.total === 0}
      <div class="muted">No activity</div>
    {:else}
      <table class="tt-table">
        <tbody>
          {#each hovered.cats as c (c.kind + c.name)}
            <tr>
              <td><span class="swatch" style:background={c.color}></span>{c.name}</td>
              <td class="num">{duration(c.ms)}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr>
            <td>Total</td>
            <td class="num">{duration(hovered.total)}</td>
          </tr>
        </tfoot>
      </table>
    {/if}
  {/if}
</Tooltip>

<style>
  .err-icon {
    display: inline-flex;
    color: var(--danger);
  }
  .page {
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    transition: opacity 0.15s;
  }
  .page.stale {
    opacity: 0.65;
  }
  .placeholder {
    border-radius: var(--radius);
    background: var(--panel);
    border: 1px solid var(--border);
  }

  /* ---- Chart ---- */
  .chart {
    position: relative;
    width: 100%;
    overflow: hidden;
  }
  svg {
    display: block;
    user-select: none;
  }
  .weekend {
    fill: var(--panel-2);
  }
  .hover-col {
    fill: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .grid {
    stroke: var(--border);
    stroke-width: 1;
    stroke-dasharray: 3 3;
    shape-rendering: crispEdges;
  }
  .grid.base {
    stroke: var(--border-strong);
    stroke-dasharray: none;
  }
  .ylabel {
    fill: var(--muted);
    font-size: 11px;
    text-anchor: end;
    font-variant-numeric: tabular-nums;
  }
  .xlabel {
    fill: var(--muted);
    font-size: 11px;
    text-anchor: middle;
    font-variant-numeric: tabular-nums;
  }
  .xlabel.sub {
    fill: var(--text-2);
  }
  .xlabel.today {
    fill: var(--accent-fg);
    font-weight: 600;
  }
  .hit {
    fill: transparent;
    cursor: pointer;
    outline: none;
  }
  .hit:focus-visible {
    stroke: var(--accent);
    stroke-width: 2;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 16px;
    margin-top: var(--sp-2);
    color: var(--text-2);
  }
  .legend-item {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .weekend-key {
    background: var(--panel-2);
    border: 1px solid var(--border-strong);
  }

  /* ---- Tooltip ---- */
  .tt-head {
    font-weight: 600;
    margin-bottom: 4px;
  }
  .tt-table {
    width: auto;
    min-width: 160px;
  }
  .tt-table td {
    padding: 1px 0;
    border: none;
  }
  .tt-table td:first-child {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-right: 16px;
  }
  .tt-table td.num {
    text-align: right;
  }
  .tt-table tfoot td {
    border-top: 1px solid var(--border);
    padding-top: 4px;
    font-weight: 600;
  }

  /* ---- Below the chart ---- */
  .tiles {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  @container (max-width: 700px) {
    .tiles {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  .cats {
    table-layout: fixed;
  }
  .cats .barcol {
    width: 30%;
  }
  .cats th.num {
    width: 88px;
  }
  .cat {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .cat .tag {
    flex: none;
  }
  @container (max-width: 560px) {
    .cats .barcol,
    .cats .perday {
      display: none;
    }
  }
</style>
