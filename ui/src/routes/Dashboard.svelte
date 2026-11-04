<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import {
    api,
    type AppUsage,
    type CategoryKind,
    type CategoryUsage,
    type KindTotal,
    type RuleDef,
    type RuleSnapshot,
    type ScopeDef,
    type SegmentKind,
    type StateResponse,
    type TimelineEntry,
  } from "../lib/api";
  import {
    addDays,
    categoryColor,
    clock,
    duration,
    kindColor,
    localDayLabel,
    percent,
    time,
  } from "../lib/format";
  import type { IconName } from "../lib/icons";
  import { errorText, notify } from "../lib/notify.svelte";
  import { usePageToolbar } from "../lib/page.svelte";
  import Card from "../lib/ui/Card.svelte";
  import Empty from "../lib/ui/Empty.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import StatTile from "../lib/ui/StatTile.svelte";
  import Tooltip from "../lib/ui/Tooltip.svelte";

  let { snap }: { snap: StateResponse | null } = $props();

  const HOUR = 3_600_000;
  const REFRESH_MS = 15_000;
  const APP_ROWS = 12;
  const KINDS: readonly CategoryKind[] = ["Work", "Game", "Web", "Other"];
  const SEG_LABEL: Record<SegmentKind, string> = {
    active: "Active",
    idle: "Idle",
    locked: "Locked",
    suspended: "Suspended",
    paused: "Paused",
  };

  // History opens a day here through sessionStorage ("ns.day"); read it once.
  function takeStoredDay(): string | null {
    try {
      const d = sessionStorage.getItem("ns.day");
      sessionStorage.removeItem("ns.day");
      return d && /^\d{4}-\d{2}-\d{2}$/.test(d) ? d : null;
    } catch {
      return null;
    }
  }

  /** The picked day, or `null` to follow today. */
  let day = $state<string | null>(takeStoredDay());
  let apps = $state<AppUsage[]>([]);
  let categories = $state<CategoryUsage[]>([]);
  let timeline = $state<TimelineEntry[]>([]);
  let kinds = $state<KindTotal[]>([]);
  let ruleDefs = $state<RuleDef[]>([]);
  /** The day the data above belongs to; `null` before the first load. */
  let loadedDay = $state<string | null>(null);
  let error = $state<string | null>(null);
  let showAllApps = $state(false);
  let acting = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;
  let seq = 0;

  const today = $derived(snap?.today ?? null);
  const shownDay = $derived(day && today && day < today ? day : today);
  const isToday = $derived(shownDay !== null && shownDay === today);
  const dataIsToday = $derived(loadedDay !== null && loadedDay === today);
  const stale = $derived(loadedDay !== null && loadedDay !== shownDay);

  async function load() {
    const d = shownDay;
    if (!d) return;
    const my = ++seq;
    api
      .rules()
      .then((r) => (ruleDefs = r.rules))
      .catch(() => {});
    try {
      const [a, c, t, k] = await Promise.all([
        api.appsUsage(d, d),
        api.categoriesUsage(d, d),
        api.timeline(d),
        api.kinds(d, d),
      ]);
      if (my !== seq) return;
      apps = a;
      categories = c;
      timeline = t;
      kinds = k;
      if (loadedDay !== d) {
        showAllApps = false;
        hover = null;
      }
      loadedDay = d;
      error = null;
    } catch (e) {
      if (my === seq) error = errorText(e);
    }
  }

  $effect(() => {
    void shownDay;
    load();
  });

  onMount(() => (timer = setInterval(load, REFRESH_MS)));
  onDestroy(() => clearInterval(timer));

  function shift(days: number) {
    if (!shownDay || !today) return;
    const next = addDays(shownDay, days);
    day = next >= today ? null : next;
  }

  function jump(e: Event & { currentTarget: HTMLInputElement }) {
    const v = e.currentTarget.value;
    if (!today || !/^\d{4}-\d{2}-\d{2}$/.test(v)) return;
    day = v >= today ? null : v;
  }

  // ---- Totals ----

  const total = $derived(categories.reduce((s, c) => s + c.ms, 0));
  const byKind = $derived.by(() => {
    const out: Record<CategoryKind, number> = { Work: 0, Game: 0, Web: 0, Other: 0 };
    for (const c of categories) out[c.kind] += c.ms;
    return out;
  });
  const kindMs = (k: SegmentKind) => kinds.filter((x) => x.kind === k).reduce((s, x) => s + x.ms, 0);
  const idleMs = $derived(kindMs("idle"));
  const lockedMs = $derived(kindMs("locked"));
  const firstActive = $derived(timeline.find((s) => s.kind === "active")?.start ?? null);

  // ---- Rules ----

  const rules = $derived(snap?.engine.rules ?? []);
  const defsById = $derived(new Map(ruleDefs.map((r) => [r.id, r])));

  function progress(r: RuleSnapshot): number {
    return r.target_ms > 0 ? Math.min(1, r.accum_ms / r.target_ms) : 0;
  }

  function barColor(p: number): string {
    if (p >= 1) return "var(--danger)";
    if (p >= 0.8) return "var(--warn)";
    return "var(--accent)";
  }

  function scopeChip(scope: ScopeDef | undefined): { text: string; color?: string } | null {
    if (scope === undefined) return null;
    if (scope === "any") return { text: "Any app" };
    if ("category" in scope) return { text: scope.category, color: categoryColor[scope.category] };
    return { text: scope.app };
  }

  function stateChip(r: RuleSnapshot, now: number): { text: string; tone: string } | null {
    if (!r.enabled) return { text: "Off", tone: "" };
    const s = r.state;
    switch (s.state) {
      case "active":
        return { text: s.level === "toast" ? "Nagging" : `Nagging (${s.level})`, tone: "danger" };
      case "pending":
        return { text: "Due", tone: "warn" };
      case "snoozed":
        return { text: `Snoozed ${clock(s.until - now)}`, tone: "accent" };
      case "in_break":
        return { text: `Break ${clock(s.until - now)}`, tone: "ok" };
      case "satisfied":
        return r.trigger === "daily_total"
          ? { text: "Over limit", tone: "danger" }
          : { text: "Skipped", tone: "" };
      default:
        return null;
    }
  }

  function scheduleText(id: string): string {
    const def = defsById.get(id);
    if (def?.trigger.kind === "schedule" && def.trigger.windows.length) {
      return `Blocked ${def.trigger.windows.map((w) => w.replace("-", "–")).join(", ")}`;
    }
    return "Schedule";
  }

  type Action = "snooze" | "skip" | "break";

  function actionsFor(r: RuleSnapshot): Action[] {
    if (!r.enabled) return [];
    const st = r.state.state;
    const out: Action[] = [];
    if (st === "active" && r.snoozes_left > 0) out.push("snooze");
    if (st === "active" || st === "snoozed") out.push("skip");
    if ((r.trigger === "interval" || r.trigger === "continuous") && st !== "in_break") {
      out.push("break");
    }
    return out;
  }

  const ACTION: Record<Action, { label: string; icon: IconName; done: string }> = {
    snooze: { label: "Snooze", icon: "snooze", done: "Snoozed" },
    skip: { label: "Skip", icon: "skip", done: "Skipped" },
    break: { label: "Start break", icon: "coffee", done: "Break started" },
  };

  async function act(r: RuleSnapshot, a: Action) {
    if (acting) return;
    acting = r.id;
    try {
      if (a === "snooze") await api.snooze(r.id);
      else if (a === "skip") await api.skip(r.id);
      else await api.startBreak(r.id);
      notify.ok(`${ACTION[a].done}: ${r.name}`);
    } catch (e) {
      notify.err(e);
    } finally {
      acting = null;
    }
  }

  // ---- Categories ----

  const catColor = (c: CategoryUsage) => c.color ?? categoryColor[c.kind];

  // ---- Timeline ----

  let trackW = $state(0);
  let trackEl = $state<HTMLDivElement | null>(null);
  let hover = $state<{ seg: TimelineEntry; x: number; y: number } | null>(null);

  const appNames = $derived(new Map(apps.map((a) => [a.app_id, a.display_name ?? a.exe_name])));

  function floorHour(ts: number): number {
    const d = new Date(ts);
    d.setMinutes(0, 0, 0);
    return d.getTime();
  }

  const span = $derived.by(() => {
    if (timeline.length === 0) return null;
    let s = Infinity;
    let e = -Infinity;
    for (const t of timeline) {
      if (t.start < s) s = t.start;
      if (t.end > e) e = t.end;
    }
    if (dataIsToday && snap) e = Math.max(e, snap.now);
    const start = floorHour(s);
    let end = floorHour(e);
    if (end < e) end += HOUR;
    if (end - start < 2 * HOUR) end = start + 2 * HOUR;
    return { start, end, len: end - start };
  });

  const pct = (ts: number) => (span ? ((ts - span.start) / span.len) * 100 : 0);

  const ticks = $derived.by(() => {
    if (!span) return [];
    const hours = span.len / HOUR;
    const labelW = time(span.start).length * 6.6 + 14;
    const perHour = trackW > 0 ? trackW / hours : 40;
    const step = [1, 2, 3, 4, 6].find((s) => perHour * s >= labelW) ?? 12;
    const out: { ts: number; pct: number; label: string | null; edge: "start" | "end" | "" }[] = [];
    for (let t = span.start; t <= span.end; t += HOUR) {
      const p = pct(t);
      const labelled = new Date(t).getHours() % step === 0;
      out.push({
        ts: t,
        pct: p,
        label: labelled ? time(t) : null,
        edge: p <= 0.01 ? "start" : p >= 99.99 ? "end" : "",
      });
    }
    return out;
  });

  function segColor(s: TimelineEntry): string {
    if (s.kind !== "active") return kindColor[s.kind];
    return categoryColor[s.category_kind ?? "Other"];
  }

  function segName(s: TimelineEntry): string {
    if (s.kind !== "active") return SEG_LABEL[s.kind];
    if (s.app_id !== null) return appNames.get(s.app_id) ?? s.exe_name ?? "Unknown app";
    return s.exe_name ?? "Unknown app";
  }

  const legend = $derived.by(() => {
    const cats = new Set<CategoryKind>();
    const segs = new Set<SegmentKind>();
    for (const s of timeline) {
      if (s.kind === "active") cats.add(s.category_kind ?? "Other");
      else segs.add(s.kind);
    }
    return [
      ...KINDS.filter((k) => cats.has(k)).map((k) => ({ id: k, label: k, color: categoryColor[k] })),
      ...(["idle", "locked", "suspended", "paused"] as const)
        .filter((k) => segs.has(k))
        .map((k) => ({ id: k, label: SEG_LABEL[k], color: kindColor[k] })),
    ];
  });

  function onTrackMove(e: PointerEvent) {
    if (!span || !trackEl) return;
    const rect = trackEl.getBoundingClientRect();
    if (rect.width <= 0) return;
    const ts = span.start + ((e.clientX - rect.left) / rect.width) * span.len;
    const tol = (3 / rect.width) * span.len;
    let best: TimelineEntry | null = null;
    let bestDist = Infinity;
    for (const s of timeline) {
      if (s.start - tol > ts) break;
      const dist = ts < s.start ? s.start - ts : ts > s.end ? ts - s.end : 0;
      if (dist <= tol && dist <= bestDist) {
        best = s;
        bestDist = dist;
      }
    }
    hover = best ? { seg: best, x: e.clientX, y: e.clientY } : null;
  }

  // ---- Apps ----

  const maxApp = $derived(Math.max(1, ...apps.map((a) => a.ms)));
  const appsTotal = $derived(apps.reduce((s, a) => s + a.ms, 0));
  const shownApps = $derived(showAllApps ? apps : apps.slice(0, APP_ROWS));

  $effect(() => usePageToolbar(tools));
</script>

{#snippet tools()}
  {#if isToday}
    <span class="pill ok live" title="Updates every 15 seconds"><span class="live-dot"></span>Live</span>
  {/if}
  {#if error}
    <span class="err-icon" title={error}><Icon name="alert" size={16} title="Could not load: {error}" /></span>
  {/if}
  {#if loadedDay !== null}
    <span class="small muted num total">{duration(total)} active</span>
  {/if}
  <div class="daynav">
    <button
      class="btn ghost icon sm"
      onclick={() => shift(-1)}
      disabled={!shownDay}
      title="Previous day"
      aria-label="Previous day"
    >
      <Icon name="chevron-left" size={16} />
    </button>
    <span class="day-label">{shownDay && today ? localDayLabel(shownDay, today) : "Today"}</span>
    <button
      class="btn ghost icon sm"
      onclick={() => shift(1)}
      disabled={!shownDay || isToday}
      title="Next day"
      aria-label="Next day"
    >
      <Icon name="chevron-right" size={16} />
    </button>
  </div>
  <input
    class="date"
    type="date"
    aria-label="Jump to day"
    title="Jump to day"
    value={shownDay ?? ""}
    max={today ?? undefined}
    disabled={!today}
    onchange={jump}
  />
{/snippet}

<div class="page" class:stale>
  <div class="tiles">
    <StatTile
      label="Active"
      value={duration(total)}
      sub={firstActive !== null ? `since ${time(firstActive)}` : `${apps.length} apps`}
    />
    {#each ["Work", "Game", "Web"] as const as k (k)}
      <StatTile
        label={k}
        value={duration(byKind[k])}
        sub={total > 0 ? `${percent(byKind[k], total)} of active` : " "}
        color={categoryColor[k]}
      />
    {/each}
    <StatTile
      label="Idle"
      value={duration(idleMs)}
      sub={lockedMs > 0 ? `Locked ${duration(lockedMs)}` : " "}
      color="var(--seg-locked)"
    />
  </div>

  <div class="grid">
    <Card title="Rules" subtitle={rules.length ? `${rules.filter((r) => r.enabled).length} of ${rules.length} on` : undefined}>
      {#if !snap}
        <div class="placeholder" style:height="120px"></div>
      {:else if rules.length === 0}
        <Empty icon="rules" title="No rules yet" hint="Rules nag you to take breaks or stop playing.">
          <a class="btn sm" href="#rules">Open Rules</a>
        </Empty>
      {:else}
        <div class="rules">
          {#each rules as r (r.id)}
            {@const p = progress(r)}
            {@const scope = scopeChip(defsById.get(r.id)?.scope)}
            {@const chip = stateChip(r, snap.now)}
            {@const actions = actionsFor(r)}
            <div class="rule" class:off={!r.enabled}>
              <div class="rule-head">
                <span class="rule-name truncate" title={r.message || r.name}>{r.name}</span>
                {#if scope}
                  <span class="tag scope">
                    {#if scope.color}<span class="swatch" style:background={scope.color}></span>{/if}
                    {scope.text}
                  </span>
                {/if}
                {#if chip}<span class="pill {chip.tone} num">{chip.text}</span>{/if}
                <span class="rule-nums num small muted">
                  {#if r.trigger !== "schedule"}
                    {duration(r.accum_ms)} / {duration(r.target_ms)}
                  {/if}
                </span>
              </div>
              {#if r.trigger === "schedule"}
                <div class="schedule small muted">
                  <Icon name="clock" size={13} />
                  <span class="truncate">{scheduleText(r.id)}</span>
                </div>
              {:else}
                <div
                  class="bar"
                  role="progressbar"
                  aria-label="{r.name} progress"
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={Math.round(p * 100)}
                >
                  <span style:width="{p * 100}%" style:background={barColor(p)}></span>
                </div>
              {/if}
              {#if actions.length}
                <div class="rule-actions">
                  {#each actions as a (a)}
                    <button
                      class="btn sm"
                      class:primary={a === "break"}
                      disabled={acting !== null}
                      title={a === "snooze" ? `${ACTION[a].label} (${r.snoozes_left} left)` : ACTION[a].label}
                      onclick={() => act(r, a)}
                    >
                      <Icon name={ACTION[a].icon} size={13} />
                      {ACTION[a].label}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </Card>

    <Card title="Categories" subtitle={total > 0 ? `${duration(total)} active` : undefined}>
      {#if loadedDay === null}
        <div class="placeholder" style:height="120px"></div>
      {:else if categories.length === 0 || total === 0}
        <Empty icon="today" title="Nothing tracked" hint={isToday ? "Time shows up here as you use apps." : "No activity on this day."} />
      {:else}
        <div class="stack" role="img" aria-label="Share of active time by category">
          {#each categories as c (c.category_id)}
            {#if c.ms > 0}
              <span
                style:flex-grow={c.ms}
                style:background={catColor(c)}
                title="{c.name}: {duration(c.ms)} ({percent(c.ms, total)})"
              ></span>
            {/if}
          {/each}
        </div>
        <ul class="cats">
          {#each categories as c (c.category_id)}
            <li>
              <span class="swatch" style:background={catColor(c)}></span>
              <span class="cat-name truncate">{c.name}</span>
              {#if c.name !== c.kind}<span class="small muted">{c.kind}</span>{/if}
              <span class="cat-ms num">{duration(c.ms)}</span>
              <span class="cat-pct num small muted">{percent(c.ms, total)}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </Card>
  </div>

  <Card
    title="Timeline"
    subtitle={span ? `${time(span.start)} – ${time(span.end)}` : undefined}
  >
    {#if loadedDay === null}
      <div class="placeholder" style:height="80px"></div>
    {:else if !span}
      <Empty icon="clock" title="No timeline" hint={isToday ? "Segments appear once you start using the computer." : "Nothing was recorded on this day."} />
    {:else}
      <div class="timeline" bind:clientWidth={trackW}>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="track"
          bind:this={trackEl}
          onpointermove={onTrackMove}
          onpointerleave={() => (hover = null)}
        >
          {#each ticks as t (t.ts)}
            <span class="hour" class:major={t.label !== null} style:left="{t.pct}%"></span>
          {/each}
          {#each timeline as s (s.id)}
            <span
              class="seg"
              class:inactive={s.kind !== "active"}
              style:left="{pct(s.start)}%"
              style:width="{((s.end - s.start) / span.len) * 100}%"
              style:background={segColor(s)}
            ></span>
          {/each}
          {#if hover}
            <span
              class="seg-hl"
              style:left="{pct(hover.seg.start)}%"
              style:width="{((hover.seg.end - hover.seg.start) / span.len) * 100}%"
            ></span>
          {/if}
          {#if dataIsToday && snap && snap.now >= span.start && snap.now <= span.end}
            <span class="now" style:left="{pct(snap.now)}%" title="Now {time(snap.now)}"></span>
          {/if}
        </div>
        <div class="axis">
          {#each ticks as t (t.ts)}
            {#if t.label}
              <span class="tick-label num {t.edge}" style:left="{t.pct}%">{t.label}</span>
            {/if}
          {/each}
        </div>
      </div>
      <div class="legend small">
        {#each legend as l (l.id)}
          <span class="legend-item"><span class="swatch" style:background={l.color}></span>{l.label}</span>
        {/each}
        {#if dataIsToday}
          <span class="legend-item"><span class="now-key"></span>Now</span>
        {/if}
      </div>
    {/if}
  </Card>

  <Card title="Apps" subtitle={apps.length ? `${apps.length} used` : undefined} padded={false}>
    {#if loadedDay === null}
      <div class="placeholder inset" style:height="160px"></div>
    {:else if apps.length === 0}
      <Empty icon="apps" title="No apps yet" hint={isToday ? "Apps you use today will be ranked here." : "No apps were used on this day."} />
    {:else}
      <table class="table apps">
        <thead>
          <tr>
            <th class="rank num">#</th>
            <th>App</th>
            <th class="barcol"><span class="sr-only">Share bar</span></th>
            <th class="num">Time</th>
            <th class="num share">Share</th>
          </tr>
        </thead>
        <tbody>
          {#each shownApps as a, i (a.app_id)}
            <tr>
              <td class="rank num muted">{i + 1}</td>
              <td class="app-cell">
                <div class="app">
                  <span class="swatch" style:background={categoryColor[a.category_kind]} title={a.category}></span>
                  <div class="app-names">
                    <span class="truncate app-name">{a.display_name ?? a.exe_name}</span>
                    {#if a.display_name && a.display_name !== a.exe_name}
                      <span class="truncate small muted">{a.exe_name}</span>
                    {/if}
                  </div>
                  {#if a.is_game}<span class="tag danger">game</span>{/if}
                </div>
              </td>
              <td class="barcol">
                <div class="bar">
                  <span style:width="{(a.ms / maxApp) * 100}%" style:background={categoryColor[a.category_kind]}></span>
                </div>
              </td>
              <td class="num">{duration(a.ms)}</td>
              <td class="num share muted">{percent(a.ms, appsTotal)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if apps.length > APP_ROWS}
        <div class="more">
          <button class="btn ghost sm" onclick={() => (showAllApps = !showAllApps)}>
            <Icon name={showAllApps ? "chevron-up" : "chevron-down"} size={14} />
            {showAllApps ? "Show top 12" : `Show all ${apps.length}`}
          </button>
        </div>
      {/if}
    {/if}
  </Card>
</div>

<!-- Outside .page: its container query makes it the containing block for fixed elements. -->
<Tooltip x={hover?.x ?? 0} y={hover?.y ?? 0} show={hover !== null}>
  {#if hover}
    {@const s = hover.seg}
    <div class="tt-head">
      <span class="swatch" style:background={segColor(s)}></span>
      <strong class="truncate">{segName(s)}</strong>
    </div>
    {#if s.kind === "active" && s.title}
      <div class="tt-title">{s.title}</div>
    {/if}
    <div class="muted num">{time(s.start)}–{time(s.end)} · {duration(s.end - s.start)}</div>
  {/if}
</Tooltip>

<style>
  /* ---- Toolbar ---- */
  .daynav {
    display: inline-flex;
    align-items: center;
    gap: 2px;
  }
  .day-label {
    min-width: 92px;
    text-align: center;
    font-weight: 600;
    white-space: nowrap;
  }
  .date {
    padding: 2px 6px;
    font-size: 12.5px;
    line-height: 20px;
  }
  .live {
    gap: 6px;
  }
  .live-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ok);
    animation: pulse 2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .err-icon {
    display: inline-flex;
    color: var(--danger);
  }

  /* ---- Layout ---- */
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
  .tiles {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: var(--sp-4);
    align-items: start;
  }
  @container (max-width: 760px) {
    .tiles {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  @container (max-width: 460px) {
    .tiles {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  .placeholder {
    border-radius: var(--radius-sm);
    background: var(--panel-2);
  }
  .placeholder.inset {
    margin: 0 16px 16px;
  }

  /* ---- Rules ---- */
  .rules {
    display: flex;
    flex-direction: column;
    margin: -4px -8px;
  }
  .rule {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    border-radius: var(--radius-sm);
  }
  .rule:hover,
  .rule:focus-within {
    background: var(--panel-2);
  }
  .rule.off .rule-name,
  .rule.off .bar {
    opacity: 0.55;
  }
  .rule-head {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    min-height: 24px;
  }
  .rule-name {
    font-weight: 500;
    min-width: 0;
  }
  .scope {
    flex: none;
  }
  .scope .swatch {
    width: 8px;
    height: 8px;
  }
  .rule-nums {
    margin-left: auto;
    flex: none;
    padding-left: 8px;
  }
  .schedule {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 6px;
    line-height: 16px;
    margin-top: -2px;
  }
  .rule-actions {
    position: absolute;
    top: 6px;
    right: 8px;
    display: flex;
    gap: 4px;
    padding-left: 16px;
    background: linear-gradient(to right, transparent, var(--panel-2) 14px);
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.12s;
  }
  .rule:hover .rule-actions,
  .rule:focus-within .rule-actions {
    opacity: 1;
    pointer-events: auto;
  }

  /* ---- Categories ---- */
  .stack {
    display: flex;
    gap: 2px;
    height: 10px;
    border-radius: 999px;
    overflow: hidden;
    margin-bottom: var(--sp-3);
  }
  .stack span {
    flex-basis: 0;
    min-width: 3px;
  }
  .cats {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .cats li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
    min-width: 0;
  }
  .cat-name {
    min-width: 0;
  }
  .cat-ms {
    margin-left: auto;
    flex: none;
  }
  .cat-pct {
    flex: none;
    width: 36px;
    text-align: right;
  }

  /* ---- Timeline ---- */
  .timeline {
    position: relative;
    padding-top: 4px;
  }
  .track {
    position: relative;
    height: 36px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
    cursor: default;
  }
  .hour {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: color-mix(in srgb, var(--border) 60%, transparent);
  }
  .hour.major {
    background: var(--border);
  }
  .seg {
    position: absolute;
    top: 4px;
    bottom: 4px;
    min-width: 1px;
  }
  .seg.inactive {
    top: 12px;
    bottom: 12px;
  }
  .seg-hl {
    position: absolute;
    top: 1px;
    bottom: 1px;
    min-width: 3px;
    border-radius: 3px;
    box-shadow:
      0 0 0 1.5px var(--text),
      0 0 0 3px var(--panel);
    pointer-events: none;
  }
  .now {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 2px;
    margin-left: -1px;
    background: var(--danger);
  }
  .now::before {
    content: "";
    position: absolute;
    top: 0;
    left: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
  }
  .axis {
    position: relative;
    height: 18px;
    margin-top: 4px;
  }
  .tick-label {
    position: absolute;
    top: 0;
    transform: translateX(-50%);
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }
  .tick-label.start {
    transform: none;
  }
  .tick-label.end {
    transform: translateX(-100%);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    margin-top: var(--sp-2);
    color: var(--text-2);
  }
  .legend-item {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .now-key {
    width: 2px;
    height: 12px;
    background: var(--danger);
    border-radius: 1px;
  }
  .tt-head {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .tt-title {
    color: var(--text-2);
    margin: 2px 0;
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  /* ---- Apps ---- */
  .apps {
    table-layout: fixed;
  }
  .apps .rank {
    width: 44px;
  }
  .apps th.rank,
  .apps td.rank {
    padding-right: 4px;
  }
  .apps .barcol {
    width: 28%;
  }
  .apps th.num:not(.rank) {
    width: 84px;
  }
  .apps .share {
    width: 64px;
  }
  .app {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .app-names {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.3;
  }
  .app-name {
    font-weight: 500;
  }
  .app .tag {
    flex: none;
  }
  .more {
    display: flex;
    justify-content: center;
    padding: 6px 0 8px;
    border-top: 1px solid var(--border);
  }
  @container (max-width: 560px) {
    .apps .barcol {
      display: none;
    }
  }
</style>
