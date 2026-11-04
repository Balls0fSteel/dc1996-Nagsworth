<script lang="ts">
  import { onMount } from "svelte";
  import { api, type HostUsage } from "../lib/api";
  import { addDays, duration, percent } from "../lib/format";
  import { usePageToolbar } from "../lib/page.svelte";
  import Card from "../lib/ui/Card.svelte";
  import Empty from "../lib/ui/Empty.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Segmented from "../lib/ui/Segmented.svelte";
  import StatTile from "../lib/ui/StatTile.svelte";

  type Range = 1 | 7 | 30;
  const ranges: readonly { id: Range; label: string }[] = [
    { id: 1, label: "Today" },
    { id: 7, label: "7 days" },
    { id: 30, label: "30 days" },
  ];

  let days = $state<Range>(1);
  let hosts = $state<HostUsage[]>([]);
  let paired = $state(false);
  let loaded = $state(false);
  let loading = $state(false);
  let error = $state<string | null>(null);
  /** Drops responses to an older range when the user clicks quickly. */
  let seq = 0;

  const ranked = $derived([...hosts].sort((a, b) => b.ms - a.ms || a.host.localeCompare(b.host)));
  const total = $derived(hosts.reduce((s, h) => s + h.ms, 0));
  const max = $derived(Math.max(1, ...hosts.map((h) => h.ms)));
  const top = $derived(ranked[0] ?? null);

  async function load() {
    const mine = ++seq;
    loading = true;
    try {
      const st = await api.state();
      const res = await api.web(addDays(st.today, -(days - 1)), st.today);
      if (mine !== seq) return;
      hosts = res;
      paired = st.extensions.length > 0;
      error = null;
      loaded = true;
    } catch (e) {
      if (mine !== seq) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (mine === seq) loading = false;
    }
  }

  function setRange(r: Range) {
    if (r === days) return;
    days = r;
    void load();
  }

  /** A stable hue per host (djb2 hash). */
  function hue(host: string): number {
    let h = 5381;
    for (let i = 0; i < host.length; i++) h = ((h << 5) + h + host.charCodeAt(i)) | 0;
    return Math.abs(h) % 360;
  }

  function letter(host: string): string {
    const bare = host.replace(/^www\./i, "");
    return (bare.match(/[\p{L}\p{N}]/u)?.[0] ?? "?").toUpperCase();
  }

  const rangeLabel = $derived(days === 1 ? "today" : `last ${days} days`);

  onMount(() => {
    void load();
  });

  $effect(() => usePageToolbar(tools));
</script>

{#snippet tools()}
  <Segmented label="Range" options={ranges} value={days} onchange={setRange} />
{/snippet}

<div class="page" class:stale={loading && loaded}>
  <div class="tiles">
    <StatTile
      label="Web time"
      value={loaded ? duration(total) : "–"}
      sub={rangeLabel}
      color="var(--cat-web)"
    />
    <StatTile
      label="Sites"
      value={loaded ? String(hosts.length) : "–"}
      sub={hosts.length === 1 ? "site visited" : "sites visited"}
    />
    <StatTile
      label="Top site"
      value={top ? top.host : "–"}
      sub={top ? `${percent(top.ms, total)} of web time, ${duration(top.ms)}` : "nothing yet"}
    />
  </div>

  <Card
    padded={false}
    title="Sites"
    subtitle="Time counts only while the browser is in front and you are not idle."
  >
    {#if error && !loaded}
      <Empty icon="alert" title="Couldn't load web activity" hint={error}>
        <button class="btn sm" onclick={load}><Icon name="refresh" size={14} /> Retry</button>
      </Empty>
    {:else if !loaded}
      <div class="placeholder" aria-busy="true" aria-label="Loading sites"></div>
    {:else if ranked.length === 0}
      {#if paired}
        <Empty
          icon="web"
          title="No sites {days === 1 ? 'today' : `in the last ${days} days`}"
          hint="The browser extension is paired; sites show up here once you browse with the browser in front."
        />
      {:else}
        <Empty
          icon="web"
          title="No web activity yet"
          hint="Time per site needs the Nagsworth browser extension. Install it and pair it with this app in Settings."
        >
          <button class="btn primary sm" onclick={() => (location.hash = "settings")}>
            <Icon name="settings" size={14} /> Open Settings
          </button>
        </Empty>
      {/if}
    {:else}
      <div class="scroll">
        <table class="table">
          <thead>
            <tr>
              <th class="num w-rank">#</th>
              <th>Site</th>
              <th class="w-bar"><span class="sr-only">Share bar</span></th>
              <th class="num w-dur">Time</th>
              <th class="num w-share">Share</th>
            </tr>
          </thead>
          <tbody>
            {#each ranked as h, i (h.host)}
              <tr>
                <td class="num muted">{i + 1}</td>
                <td>
                  <div class="site">
                    <span class="avatar" aria-hidden="true" style:background="hsl({hue(h.host)} 55% 46%)">
                      {letter(h.host)}
                    </span>
                    <span class="truncate" title={h.host}>{h.host}</span>
                  </div>
                </td>
                <td>
                  <div class="bar">
                    <span style:width="{(h.ms / max) * 100}%" style:background="var(--cat-web)"></span>
                  </div>
                </td>
                <td class="num">{duration(h.ms)}</td>
                <td class="num muted">{percent(h.ms, total)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </Card>
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: var(--sp-4);
    transition: opacity 0.15s;
  }
  .page.stale {
    opacity: 0.6;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--sp-3);
  }
  @media (max-width: 640px) {
    .tiles {
      grid-template-columns: minmax(0, 1fr);
    }
  }
  .tiles :global(.value) {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .placeholder {
    min-height: 220px;
  }
  .scroll {
    overflow-x: auto;
  }
  .table {
    min-width: 520px;
    table-layout: fixed;
  }
  .w-rank {
    width: 44px;
  }
  .w-bar {
    width: 32%;
  }
  .w-dur {
    width: 90px;
  }
  .w-share {
    width: 70px;
  }
  .site {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .avatar {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    color: white;
    font-size: 12px;
    font-weight: 600;
    line-height: 1;
  }
</style>
