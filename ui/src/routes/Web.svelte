<script lang="ts">
  import { onMount } from "svelte";
  import { api, type HostUsage } from "../lib/api";
  import { addDays, duration } from "../lib/format";

  let days = $state(1);
  let hosts = $state<HostUsage[]>([]);
  let error = $state<string | null>(null);

  const max = $derived(Math.max(1, ...hosts.map((h) => h.ms)));

  async function load() {
    try {
      const { today } = await api.state();
      hosts = await api.web(addDays(today, -(days - 1)), today);
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(load);
</script>

<section class="panel">
  <div class="head">
    <h2>Websites</h2>
    <select bind:value={days} onchange={load}>
      <option value={1}>Today</option>
      <option value={7}>Last 7 days</option>
      <option value={30}>Last 30 days</option>
    </select>
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  <p class="muted">
    Needs the browser extension (pair it in Settings). Time counts only while the browser is in front
    and you are not idle.
  </p>
  <table>
    <tbody>
      {#each hosts as h (h.host)}
        <tr>
          <td>{h.host}</td>
          <td class="barcell"><div class="bar"><span style:width="{(h.ms / max) * 100}%" style:background="var(--cat-web)"></span></div></td>
          <td>{duration(h.ms)}</td>
        </tr>
      {:else}
        <tr><td class="muted">No web visits recorded.</td></tr>
      {/each}
    </tbody>
  </table>
</section>

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .barcell {
    width: 55%;
  }
</style>
