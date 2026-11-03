<script lang="ts">
  import { onMount } from "svelte";
  import { api, type NagLogEntry } from "../lib/api";
  import { dateTime } from "../lib/format";

  let text = $state("");
  let saved = $state("");
  let error = $state<string | null>(null);
  let message = $state<string | null>(null);
  let nags = $state<NagLogEntry[]>([]);

  async function load() {
    try {
      const r = await api.rules();
      text = saved = r.toml;
      error = r.error ?? null;
      nags = await api.nags(50);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function save() {
    message = null;
    try {
      await api.saveRules(text);
      saved = text;
      error = null;
      message = "Saved and applied.";
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(load);
</script>

<section class="panel">
  <div class="head">
    <h2>Config and rules</h2>
    <div>
      <button onclick={() => (text = saved)} disabled={text === saved}>Revert</button>
      <button class="primary" onclick={save} disabled={text === saved}>Save</button>
    </div>
  </div>
  <p class="muted">
    The whole config file. Rules are <code>[[rules]]</code> tables; the comments explain every trigger.
    Saving validates first and applies without a restart.
  </p>
  {#if error}<pre class="error">{error}</pre>{/if}
  {#if message}<p class="ok">{message}</p>{/if}
  <textarea bind:value={text} spellcheck="false"></textarea>
</section>

<section class="panel" style="margin-top: 16px">
  <h2>Recent nags</h2>
  <table>
    <tbody>
      {#each nags as n (n.id)}
        <tr>
          <td class="muted">{dateTime(n.at)}</td>
          <td>{n.rule_id}</td>
          <td>{n.kind}</td>
          <td class="muted">{n.level ?? n.reason ?? ""}</td>
        </tr>
      {:else}
        <tr><td class="muted">No nags yet.</td></tr>
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
  .head div {
    display: flex;
    gap: 8px;
  }
  textarea {
    width: 100%;
    min-height: 480px;
    font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
    font-size: 13px;
  }
  pre.error {
    white-space: pre-wrap;
  }
  .ok {
    color: var(--ok);
  }
</style>
