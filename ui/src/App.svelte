<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type StateResponse } from "./lib/api";
  import { duration } from "./lib/format";
  import Dashboard from "./routes/Dashboard.svelte";
  import Apps from "./routes/Apps.svelte";
  import Web from "./routes/Web.svelte";
  import Rules from "./routes/Rules.svelte";
  import Settings from "./routes/Settings.svelte";

  const tabs = [
    { id: "dashboard", label: "Today" },
    { id: "apps", label: "Apps" },
    { id: "web", label: "Web" },
    { id: "rules", label: "Rules" },
    { id: "settings", label: "Settings" },
  ] as const;
  type TabId = (typeof tabs)[number]["id"];

  const fromHash = (): TabId => {
    const h = location.hash.slice(1);
    return (tabs.find((t) => t.id === h)?.id ?? "dashboard") as TabId;
  };

  let tab = $state<TabId>(fromHash());
  let snap = $state<StateResponse | null>(null);
  let error = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try {
      snap = await api.state();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function go(id: TabId) {
    tab = id;
    location.hash = id;
  }

  async function togglePause() {
    if (snap?.tracking_paused) await api.resume();
    else await api.pause(30);
    await refresh();
  }

  async function toggleReading() {
    await api.readingMode(!snap?.engine.reading_mode);
    await refresh();
  }

  onMount(() => {
    refresh();
    timer = setInterval(refresh, 2000);
    window.addEventListener("hashchange", () => (tab = fromHash()));
  });
  onDestroy(() => clearInterval(timer));

  const activity = $derived(snap?.activity);
  const kind = $derived(activity?.kind ?? null);
</script>

<header>
  <div class="brand">Nagsworth</div>
  <nav>
    {#each tabs as t (t.id)}
      <button class:active={tab === t.id} onclick={() => go(t.id)}>{t.label}</button>
    {/each}
  </nav>
  <div class="status">
    {#if error}
      <span class="error" title={error}>API unreachable</span>
    {:else if snap}
      <span class="dot {kind ?? 'none'}"></span>
      {#if kind === "active" && activity?.app}
        <span title={activity.title ?? ""}>{activity.app.exe_name}</span>
      {:else if kind === "idle"}
        <span class="muted">idle {duration(activity?.idle_ms ?? 0)}</span>
      {:else}
        <span class="muted">{kind ?? "starting"}</span>
      {/if}
      <button onclick={toggleReading} class:on={snap.engine.reading_mode} title="Break timers keep running without input">
        Reading mode
      </button>
      <button onclick={togglePause}>{snap.tracking_paused ? "Resume" : "Pause 30 min"}</button>
      <button class="primary" onclick={() => api.startBreak()}>Break now</button>
    {/if}
  </div>
</header>

<main>
  {#if tab === "dashboard"}
    <Dashboard {snap} />
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

<style>
  header {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 20px;
    background: var(--panel);
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    z-index: 1;
    flex-wrap: wrap;
  }
  .brand {
    font-weight: 700;
  }
  nav {
    display: flex;
    gap: 4px;
  }
  nav button {
    border-color: transparent;
    background: transparent;
  }
  nav button.active {
    background: var(--bg);
    border-color: var(--border);
  }
  .status {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .status button.on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--seg-idle);
  }
  .dot.active {
    background: var(--ok);
  }
  .dot.paused {
    background: var(--seg-paused);
  }
  main {
    padding: 20px;
    max-width: 1100px;
    margin: 0 auto;
  }
</style>
