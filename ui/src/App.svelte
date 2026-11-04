<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type StateResponse } from "./lib/api";
  import { duration } from "./lib/format";
  import type { IconName } from "./lib/icons";
  import { notify } from "./lib/notify.svelte";
  import { pageToolbar } from "./lib/page.svelte";
  import Icon from "./lib/ui/Icon.svelte";
  import Switch from "./lib/ui/Switch.svelte";
  import Toasts from "./lib/ui/Toasts.svelte";
  import Dashboard from "./routes/Dashboard.svelte";
  import History from "./routes/History.svelte";
  import Apps from "./routes/Apps.svelte";
  import Web from "./routes/Web.svelte";
  import Rules from "./routes/Rules.svelte";
  import Settings from "./routes/Settings.svelte";

  const tabs: readonly { id: string; label: string; icon: IconName }[] = [
    { id: "dashboard", label: "Today", icon: "today" },
    { id: "history", label: "History", icon: "history" },
    { id: "apps", label: "Apps", icon: "apps" },
    { id: "web", label: "Web", icon: "web" },
    { id: "rules", label: "Rules", icon: "rules" },
    { id: "settings", label: "Settings", icon: "settings" },
  ];

  const POLL_MS = 2000;
  const TOTAL_EVERY_MS = 30_000;

  const fromHash = (): string => {
    const h = location.hash.slice(1);
    return tabs.find((t) => t.id === h)?.id ?? "dashboard";
  };

  let tab = $state(fromHash());
  let snap = $state<StateResponse | null>(null);
  let error = $state<string | null>(null);
  let todayTotal = $state<number | null>(null);
  let totalAt = 0;
  let totalDay: string | null = null;
  let busy = $state(false);
  let timer: ReturnType<typeof setInterval> | undefined;

  const current = $derived(tabs.find((t) => t.id === tab) ?? tabs[0]);

  async function refreshTotal(today: string) {
    try {
      const apps = await api.appsUsage(today, today);
      todayTotal = apps.reduce((s, a) => s + a.ms, 0);
      totalAt = Date.now();
      totalDay = today;
    } catch {
      // The state poll reports connection problems.
    }
  }

  async function refresh() {
    try {
      snap = await api.state();
      error = null;
      if (totalDay !== snap.today || Date.now() - totalAt >= TOTAL_EVERY_MS) {
        void refreshTotal(snap.today);
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function control(action: () => Promise<unknown>, done?: string) {
    if (busy) return;
    busy = true;
    try {
      await action();
      if (done) notify.ok(done);
      await refresh();
    } catch (e) {
      notify.err(e);
    } finally {
      busy = false;
    }
  }

  const breakNow = () => control(() => api.startBreak(), "Break started");
  const togglePause = () =>
    snap?.tracking_paused
      ? control(() => api.resume(), "Tracking resumed")
      : control(() => api.pause(30), "Paused for 30 minutes");
  const setReading = (on: boolean) =>
    control(() => api.readingMode(on), on ? "Reading mode on" : "Reading mode off");

  function isEditable(el: Element | null): boolean {
    if (!(el instanceof HTMLElement)) return false;
    return (
      el instanceof HTMLInputElement ||
      el instanceof HTMLTextAreaElement ||
      el instanceof HTMLSelectElement ||
      el.isContentEditable
    );
  }

  function onKey(e: KeyboardEvent) {
    if (e.ctrlKey || e.altKey || e.metaKey || e.repeat) return;
    if (isEditable(document.activeElement)) return;
    const n = Number(e.key);
    if (Number.isInteger(n) && n >= 1 && n <= tabs.length) {
      e.preventDefault();
      location.hash = tabs[n - 1].id;
    }
  }

  const onHash = () => (tab = fromHash());

  onMount(() => {
    refresh();
    timer = setInterval(refresh, POLL_MS);
    window.addEventListener("hashchange", onHash);
  });
  onDestroy(() => {
    clearInterval(timer);
    window.removeEventListener("hashchange", onHash);
  });

  // Live status line.
  const status = $derived.by((): { tone: string; text: string; title?: string } => {
    if (!snap) return { tone: "none", text: "Connecting" };
    if (snap.tracking_paused) return { tone: "paused", text: "Paused" };
    const a = snap.activity;
    switch (a.kind) {
      case "active":
        return a.app
          ? { tone: "active", text: `Active in ${a.app.exe_name}`, title: a.title ?? undefined }
          : { tone: "active", text: "Active" };
      case "idle":
        return { tone: "idle", text: `Idle ${duration(a.idle_ms)}` };
      case "locked":
        return { tone: "locked", text: "Locked" };
      case "suspended":
        return { tone: "locked", text: "Suspended" };
      case "paused":
        return { tone: "paused", text: "Paused" };
      default:
        return { tone: "none", text: "Starting" };
    }
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="shell">
  <aside class="sidebar">
    <div class="brand">
      <img src="/icon.png" alt="" width="24" height="24" />
      <span class="label">Nagsworth</span>
    </div>

    <nav aria-label="Pages">
      {#each tabs as t, i (t.id)}
        <a
          href="#{t.id}"
          class="nav-item"
          class:active={tab === t.id}
          aria-current={tab === t.id ? "page" : undefined}
          title="{t.label} ({i + 1})"
        >
          <Icon name={t.icon} size={18} />
          <span class="label">{t.label}</span>
        </a>
      {/each}
    </nav>

    <div class="status-block">
      <div class="status" title={status.title ?? status.text}>
        <span class="dot {status.tone}"></span>
        <div class="label status-text">
          <div class="truncate">{status.text}</div>
          <div class="small muted num">
            {todayTotal === null ? " " : `${duration(todayTotal)} today`}
          </div>
        </div>
      </div>

      <div class="controls">
        <button class="btn primary" onclick={breakNow} disabled={!snap || busy} title="Break now">
          <Icon name="coffee" size={16} />
          <span class="label">Break now</span>
        </button>
        <button
          class="btn"
          onclick={togglePause}
          disabled={!snap || busy}
          title={snap?.tracking_paused ? "Resume tracking" : "Pause tracking for 30 minutes"}
        >
          <Icon name={snap?.tracking_paused ? "play" : "pause"} size={16} />
          <span class="label">{snap?.tracking_paused ? "Resume" : "Pause 30 min"}</span>
        </button>
        <div class="reading label">
          <Switch
            checked={snap?.engine.reading_mode ?? false}
            disabled={!snap || busy}
            label="Reading mode"
            title="Break timers keep running without input"
            onchange={setReading}
          />
        </div>
        <button
          class="btn icon reading-rail"
          class:on={snap?.engine.reading_mode}
          onclick={() => setReading(!snap?.engine.reading_mode)}
          disabled={!snap || busy}
          title="Reading mode: {snap?.engine.reading_mode ? 'on' : 'off'}"
          aria-pressed={snap?.engine.reading_mode ?? false}
        >
          <Icon name="book" size={16} />
        </button>
      </div>
    </div>
  </aside>

  <div class="main-wrap">
    {#if error}
      <div class="conn banner danger" role="alert" title={error}>
        <Icon name="alert" size={16} />
        <span>Can't reach the Nagsworth service. Retrying every 2 s.</span>
      </div>
    {/if}

    <main>
      <div class="page-head">
        <h1>{current.label}</h1>
        {#if pageToolbar.current}
          <div class="page-tools">{@render pageToolbar.current()}</div>
        {/if}
      </div>

      {#if tab === "dashboard"}
        <Dashboard {snap} />
      {:else if tab === "history"}
        <History />
      {:else if tab === "apps"}
        <Apps />
      {:else if tab === "web"}
        <Web />
      {:else if tab === "rules"}
        <Rules />
      {:else}
        <Settings {snap} />
      {/if}
    </main>
  </div>
</div>

<Toasts />

<style>
  .shell {
    display: flex;
    min-height: 100vh;
  }

  /* ---- Sidebar ---- */
  .sidebar {
    position: sticky;
    top: 0;
    height: 100vh;
    width: 220px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px 10px 12px;
    background: var(--panel);
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 2px 8px 12px;
    font-weight: 650;
    font-size: 15px;
    letter-spacing: -0.01em;
  }
  .brand img {
    flex: none;
    border-radius: 6px;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    color: var(--text-2);
    font-weight: 500;
    text-decoration: none;
    transition:
      background-color 0.12s,
      color 0.12s;
  }
  .nav-item:hover {
    background: color-mix(in srgb, var(--text) 6%, transparent);
    color: var(--text);
    text-decoration: none;
  }
  .nav-item.active {
    background: var(--accent-soft);
    color: var(--accent-fg);
  }
  .nav-item:focus-visible {
    box-shadow: var(--focus);
  }

  .status-block {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 8px 0;
    border-top: 1px solid var(--border);
  }
  .status {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    min-width: 0;
  }
  .status-text {
    min-width: 0;
    line-height: 1.35;
    font-weight: 500;
  }
  .dot {
    flex: none;
    width: 9px;
    height: 9px;
    margin-top: 5px;
    border-radius: 50%;
    background: var(--seg-idle);
  }
  .dot.active {
    background: var(--ok);
    box-shadow: 0 0 0 3px var(--ok-soft);
  }
  .dot.idle {
    background: var(--warn);
  }
  .dot.paused {
    background: var(--seg-paused);
  }
  .dot.locked {
    background: var(--seg-locked);
  }
  .controls {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .controls .btn {
    justify-content: flex-start;
  }
  .reading {
    padding: 4px 2px 0;
    font-size: 13px;
    color: var(--text-2);
  }
  .reading-rail {
    display: none;
  }
  .reading-rail.on {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent-fg);
  }

  /* ---- Main ---- */
  .main-wrap {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .conn {
    margin: 16px 24px 0;
    max-width: 1200px;
  }
  main {
    width: 100%;
    max-width: 1248px;
    padding: 20px 24px 32px;
  }
  .page-head {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 12px;
    min-height: 36px;
    margin-bottom: 16px;
  }
  .page-head h1 {
    flex: 1;
    min-width: 0;
  }
  .page-tools {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }

  /* ---- Icon rail under 900 px ---- */
  @media (max-width: 899px) {
    .sidebar {
      width: 60px;
      padding: 12px 8px;
      align-items: center;
    }
    .sidebar .label {
      display: none;
    }
    .brand {
      padding: 2px 0 10px;
    }
    nav {
      align-items: center;
    }
    .nav-item {
      padding: 9px;
    }
    .status-block {
      padding: 10px 0 0;
      align-items: center;
    }
    .status {
      justify-content: center;
    }
    .dot {
      margin-top: 0;
    }
    .controls {
      align-items: center;
    }
    .controls .btn {
      width: 36px;
      height: 36px;
      padding: 0;
      justify-content: center;
    }
    .reading-rail {
      display: inline-flex;
    }
    main {
      padding: 16px;
    }
    .conn {
      margin: 12px 16px 0;
    }
  }
</style>
