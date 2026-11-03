<script lang="ts">
  // Standalone pairing page, opened from the tray.
  import { onDestroy, onMount } from "svelte";
  import { api } from "./lib/api";
  import { clock } from "./lib/format";

  let code = $state<{ code: string; expires_at: number } | null>(null);
  let now = $state(Date.now());
  let error = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;

  async function fresh() {
    try {
      code = await api.startPairing();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(() => {
    fresh();
    timer = setInterval(() => (now = Date.now()), 500);
  });
  onDestroy(() => clearInterval(timer));

  const left = $derived(code ? code.expires_at - now : 0);
</script>

<main>
  <h1>Pair the browser extension</h1>
  <p class="muted">Open the extension's options page and enter this code.</p>
  {#if code && left > 0}
    <div class="code">{code.code}</div>
    <p class="muted">expires in {clock(left)}</p>
  {:else if code}
    <p>The code expired.</p>
  {/if}
  <button onclick={fresh}>New code</button>
  {#if error}<p class="error">{error}</p>{/if}
</main>

<style>
  main {
    padding: 24px;
    text-align: center;
  }
  h1 {
    font-size: 18px;
  }
  .code {
    font-size: 44px;
    letter-spacing: 10px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }
</style>
