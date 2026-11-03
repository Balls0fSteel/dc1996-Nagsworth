<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    apiInfo,
    downloadExport,
    getAutostart,
    setAutostart,
    type StateResponse,
  } from "../lib/api";
  import { addDays, clock, dateTime } from "../lib/format";

  let { snap }: { snap: StateResponse | null } = $props();

  let baseUrl = $state("");
  let code = $state<{ code: string; expires_at: number } | null>(null);
  let error = $state<string | null>(null);

  let autostart = $state<boolean | null>(null);
  let exportDays = $state(30);
  let backupPath = $state<string | null>(null);

  onMount(async () => {
    baseUrl = (await apiInfo()).base_url;
    try {
      autostart = await getAutostart();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  });

  async function toggleAutostart(on: boolean) {
    try {
      await setAutostart(on);
      autostart = await getAutostart();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function exportData(format: "csv" | "json") {
    if (!snap) return;
    try {
      await downloadExport(format, addDays(snap.today, -(exportDays - 1)), snap.today);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function backup() {
    try {
      backupPath = (await api.backup()).path;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function pair() {
    try {
      code = await api.startPairing();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  const left = $derived(code && snap ? code.expires_at - snap.now : 0);
</script>

<section class="panel">
  <h2>Browser extension</h2>
  <p class="muted">
    Install the extension (Chrome or Firefox, from <code>extension/dist</code>), open its options and
    enter the code below. Codes work once and expire after 5 minutes.
  </p>
  <button class="primary" onclick={pair}>Show pairing code</button>
  {#if code && left > 0}
    <div class="code">{code.code}</div>
    <p class="muted">expires in {clock(left)}</p>
  {/if}
  {#if error}<p class="error">{error}</p>{/if}

  <h2 style="margin-top: 20px">Paired browsers</h2>
  <table>
    <tbody>
      {#each snap?.extensions ?? [] as [id, c] (id)}
        <tr>
          <td>{c.browser}</td>
          <td class="muted">{c.version ?? ""}</td>
          <td class="muted">last seen {dateTime(c.last_seen)}</td>
        </tr>
      {:else}
        <tr><td class="muted">No extension has checked in since the app started.</td></tr>
      {/each}
    </tbody>
  </table>
</section>

<section class="panel" style="margin-top: 16px">
  <h2>App</h2>
  {#if autostart !== null}
    <label>
      <input type="checkbox" checked={autostart} onchange={(e) => toggleAutostart(e.currentTarget.checked)} />
      Start when you log in (hidden, in the tray)
    </label>
  {/if}
  <h2 style="margin-top: 16px">Your data</h2>
  <div class="row">
    <select bind:value={exportDays}>
      <option value={1}>Today</option>
      <option value={7}>Last 7 days</option>
      <option value={30}>Last 30 days</option>
      <option value={365}>Last year</option>
    </select>
    <button onclick={() => exportData("csv")}>Export CSV</button>
    <button onclick={() => exportData("json")}>Export JSON</button>
    <button onclick={backup}>Back up database</button>
  </div>
  {#if backupPath}<p class="muted">Saved to <code>{backupPath}</code></p>{/if}
</section>

<section class="panel" style="margin-top: 16px">
  <h2>Local API</h2>
  <p><code>{baseUrl}</code></p>
  <p class="muted">
    Bound to localhost only. Every route except <code>/health</code> and <code>/pair</code> needs the
    bearer token. Settings such as idle threshold, title storage and retention are in the config file
    (Rules tab).
  </p>
</section>

<style>
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .code {
    font-size: 40px;
    letter-spacing: 8px;
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    margin: 12px 0 0;
  }
</style>
