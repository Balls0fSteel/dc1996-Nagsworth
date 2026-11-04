<script lang="ts">
  // Standalone pairing page, opened from the tray.
  import { onDestroy, onMount } from "svelte";
  import { api } from "./lib/api";
  import { clock } from "./lib/format";
  import Icon from "./lib/ui/Icon.svelte";

  // A pairing code is valid for five minutes.
  const VALID_MS = 5 * 60_000;

  let code = $state<{ code: string; expires_at: number } | null>(null);
  let now = $state(Date.now());
  let error = $state<string | null>(null);
  let loading = $state(false);
  let copied = $state(false);
  let timer: ReturnType<typeof setInterval> | undefined;
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  async function fresh() {
    loading = true;
    try {
      code = await api.startPairing();
      now = Date.now();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function copy() {
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code.code);
      copied = true;
      clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => (copied = false), 1500);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  onMount(() => {
    fresh();
    timer = setInterval(() => (now = Date.now()), 500);
  });
  onDestroy(() => {
    clearInterval(timer);
    clearTimeout(copiedTimer);
  });

  const left = $derived(code ? Math.max(0, code.expires_at - now) : 0);
  const live = $derived(code !== null && left > 0);
  const fraction = $derived(Math.min(1, left / VALID_MS));
</script>

<main>
  <h1>Pair the browser extension</h1>
  <ol class="steps">
    <li><span class="n">1</span>Open the Nagsworth extension's options page</li>
    <li><span class="n">2</span>Enter this code</li>
  </ol>

  {#if code && live}
    <div class="code-row">
      <span class="code num" aria-label="Pairing code {code.code.split('').join(' ')}">
        {code.code}
      </span>
      <div class="code-actions">
        <button
          class="btn ghost icon"
          onclick={copy}
          aria-label="Copy code"
          title="Copy code"
        >
          <Icon name={copied ? "check" : "copy"} size={16} />
        </button>
        <button
          class="btn ghost icon"
          onclick={fresh}
          disabled={loading}
          aria-label="Get a new code"
          title="New code"
        >
          <Icon name="refresh" size={16} />
        </button>
      </div>
    </div>
    <div class="validity">
      <div
        class="bar"
        role="progressbar"
        aria-label="Code validity"
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(fraction * 100)}
      >
        <span style:width="{fraction * 100}%" class:low={left < 60_000}></span>
      </div>
      <p class="meta small">
        <span class="muted num">expires in {clock(left)}</span>
        <span class="copied" role="status">{copied ? "Copied" : ""}</span>
      </p>
    </div>
  {:else if code}
    <div class="expired">
      <p class="muted">The code expired.</p>
      <button class="btn ghost" onclick={fresh} disabled={loading}>
        <Icon name="refresh" size={14} />
        New code
      </button>
    </div>
  {:else if !error}
    <p class="muted placeholder">Getting a code…</p>
  {/if}

  {#if error}
    <p class="error small" role="alert">
      <Icon name="alert" size={14} />
      {error}
      {#if !code}
        <button class="btn ghost sm" onclick={fresh} disabled={loading}>Try again</button>
      {/if}
    </p>
  {/if}
</main>

<style>
  :global(body.page-pair) {
    overflow: hidden;
  }

  main {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 20px 28px;
    text-align: center;
  }

  h1 {
    font-size: 18px;
  }

  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
    text-align: left;
    color: var(--text-2);
  }
  .steps li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .n {
    display: grid;
    place-items: center;
    flex: none;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--accent-soft);
    color: var(--accent-fg);
    font-size: 11.5px;
    font-weight: 600;
  }

  .code-row {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .code {
    font-size: 44px;
    line-height: 1.15;
    font-weight: 700;
    letter-spacing: 0.18em;
    /* Balance the trailing letter spacing so the digits stay centered. */
    margin-right: -0.18em;
    padding: 2px 12px;
    border-radius: var(--radius);
    background: var(--panel-2);
    border: 1px solid var(--border);
    user-select: all;
  }
  .code-actions {
    position: absolute;
    left: 100%;
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-left: 6px;
  }

  .validity {
    width: 220px;
  }
  .bar > span.low {
    background: var(--warn);
  }
  .meta {
    display: flex;
    justify-content: space-between;
    margin: 6px 0 0;
  }
  .copied {
    color: var(--ok);
    font-weight: 500;
  }

  .expired {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    min-height: 82px;
    justify-content: center;
  }
  .expired p,
  .placeholder {
    margin: 0;
  }

  .error {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    max-width: 100%;
  }
</style>
