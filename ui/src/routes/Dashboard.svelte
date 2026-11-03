<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    api,
    type AppUsage,
    type CategoryUsage,
    type RuleSnapshot,
    type StateResponse,
    type TimelineEntry,
  } from "../lib/api";
  import { addDays, categoryColor, clock, duration, kindColor, time } from "../lib/format";

  let { snap }: { snap: StateResponse | null } = $props();

  let day = $state<string | null>(null);
  let apps = $state<AppUsage[]>([]);
  let categories = $state<CategoryUsage[]>([]);
  let timeline = $state<TimelineEntry[]>([]);
  let error = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  const today = $derived(snap?.today ?? null);
  const shownDay = $derived(day ?? today);
  const total = $derived(apps.reduce((s, a) => s + a.ms, 0));
  const maxApp = $derived(Math.max(1, ...apps.map((a) => a.ms)));

  async function load() {
    if (!shownDay) return;
    try {
      [apps, categories, timeline] = await Promise.all([
        api.appsUsage(shownDay, shownDay),
        api.categoriesUsage(shownDay, shownDay),
        api.timeline(shownDay),
      ]);
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  $effect(() => {
    void shownDay;
    load();
  });

  onMount(() => (timer = setInterval(load, 15_000)));
  onDestroy(() => clearInterval(timer));

  function shift(days: number) {
    if (!shownDay || !today) return;
    const next = addDays(shownDay, days);
    day = next >= today ? null : next;
  }

  // Timeline geometry: the span from the first segment to the last.
  const span = $derived.by(() => {
    if (timeline.length === 0) return null;
    const start = timeline[0].start;
    const end = Math.max(...timeline.map((s) => s.end));
    return { start, end, len: Math.max(1, end - start) };
  });

  function segColor(s: TimelineEntry): string {
    if (s.kind !== "active") return kindColor[s.kind];
    return categoryColor[s.category_kind ?? "Other"];
  }

  function progress(r: RuleSnapshot): number {
    return r.target_ms > 0 ? Math.min(1, r.accum_ms / r.target_ms) : 0;
  }

  function stateLabel(r: RuleSnapshot, now: number): string {
    const s = r.state;
    switch (s.state) {
      case "active":
        return `nagging (${s.level})`;
      case "snoozed":
        return `snoozed ${clock(s.until - now)}`;
      case "in_break":
        return `break ${clock(s.until - now)}`;
      case "pending":
        return "due";
      case "satisfied":
        return "over limit";
      default:
        return r.enabled ? "" : "disabled";
    }
  }
</script>

<div class="toolbar">
  <button onclick={() => shift(-1)}>&larr;</button>
  <strong>{shownDay === today ? "Today" : shownDay}</strong>
  <button onclick={() => shift(1)} disabled={!day}>&rarr;</button>
  <span class="muted">{duration(total)} active</span>
  {#if error}<span class="error">{error}</span>{/if}
</div>

<div class="grid">
  <section class="panel">
    <h2>Categories</h2>
    {#if categories.length === 0}
      <p class="muted">Nothing tracked yet.</p>
    {/if}
    {#each categories as c (c.category_id)}
      <div class="row">
        <span class="swatch" style:background={c.color ?? categoryColor[c.kind]}></span>
        <span class="name">{c.name}</span>
        <span class="value">{duration(c.ms)}</span>
      </div>
      <div class="bar">
        <span style:width="{(c.ms / Math.max(1, total)) * 100}%" style:background={c.color ?? categoryColor[c.kind]}></span>
      </div>
    {/each}
  </section>

  <section class="panel">
    <h2>Rules</h2>
    {#each snap?.engine.rules ?? [] as r (r.id)}
      <div class="rule">
        <div class="row">
          <span class="name">{r.name}</span>
          <span class="muted small">{stateLabel(r, snap?.now ?? 0)}</span>
          <span class="value">
            {#if r.trigger === "schedule"}
              schedule
            {:else}
              {duration(r.accum_ms)} / {duration(r.target_ms)}
            {/if}
          </span>
        </div>
        {#if r.trigger !== "schedule"}
          <div class="bar">
            <span style:width="{progress(r) * 100}%" style:background={progress(r) >= 1 ? "var(--danger)" : "var(--accent)"}></span>
          </div>
        {/if}
      </div>
    {:else}
      <p class="muted">No rules.</p>
    {/each}
  </section>

  <section class="panel wide">
    <h2>Timeline</h2>
    {#if span}
      <div class="timeline">
        {#each timeline as s (s.id)}
          <span
            title="{time(s.start)}–{time(s.end)} {s.exe_name ?? s.kind}{s.title ? ` — ${s.title}` : ''}"
            style:left="{((s.start - span.start) / span.len) * 100}%"
            style:width="{Math.max(0.1, ((s.end - s.start) / span.len) * 100)}%"
            style:background={segColor(s)}
          ></span>
        {/each}
      </div>
      <div class="axis muted small">
        <span>{time(span.start)}</span><span>{time(span.end)}</span>
      </div>
    {:else}
      <p class="muted">No segments for this day.</p>
    {/if}
  </section>

  <section class="panel wide">
    <h2>Apps</h2>
    <table>
      <tbody>
        {#each apps as a (a.app_id)}
          <tr>
            <td class="app">
              <span class="swatch" style:background={categoryColor[a.category_kind]}></span>
              {a.display_name ?? a.exe_name}
              {#if a.is_game}<span class="tag">game</span>{/if}
            </td>
            <td class="barcell">
              <div class="bar"><span style:width="{(a.ms / maxApp) * 100}%" style:background={categoryColor[a.category_kind]}></span></div>
            </td>
            <td class="value">{duration(a.ms)}</td>
          </tr>
        {:else}
          <tr><td class="muted">Nothing tracked yet.</td></tr>
        {/each}
      </tbody>
    </table>
  </section>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 16px;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
  }
  .wide {
    grid-column: 1 / -1;
  }
  @media (max-width: 760px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 0 4px;
  }
  .name {
    flex: 1;
  }
  .value {
    font-variant-numeric: tabular-nums;
  }
  .small {
    font-size: 12px;
  }
  .swatch {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 3px;
    flex: none;
  }
  .rule + .rule {
    margin-top: 10px;
  }
  .timeline {
    position: relative;
    height: 28px;
    background: var(--bg);
    border-radius: 6px;
    overflow: hidden;
  }
  .timeline span {
    position: absolute;
    top: 0;
    bottom: 0;
  }
  .axis {
    display: flex;
    justify-content: space-between;
    margin-top: 4px;
  }
  .app {
    display: flex;
    align-items: center;
    gap: 8px;
    white-space: nowrap;
  }
  .barcell {
    width: 55%;
  }
  .tag {
    font-size: 11px;
    padding: 0 6px;
    border-radius: 8px;
    background: var(--cat-game);
    color: white;
  }
</style>
