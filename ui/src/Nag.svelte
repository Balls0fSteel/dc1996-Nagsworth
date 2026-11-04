<script lang="ts">
  // The toast and overlay windows. The app opens them when a rule nags and
  // closes them when the nag is dismissed; this page only renders the nags
  // and sends the user's choice back.
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { clock, duration } from "./lib/format";
  import Icon from "./lib/ui/Icon.svelte";

  interface NagView {
    rule_id: string;
    rule_name: string;
    message: string;
    level: "toast" | "overlay" | "lock";
    since: number;
    break_for_ms: number | null;
    snoozes_left: number;
    break_until: number | null;
  }

  type Action = "snooze" | "skip" | "break";

  let { mode }: { mode: "toast" | "overlay" } = $props();

  let nags = $state<NagView[]>([]);
  let now = $state(Date.now());
  let busy = $state(false);
  // Content height of the toast stack; the app sizes the window to it.
  let stackHeight = $state(0);
  let reported = 0;
  let unlisten: UnlistenFn | undefined;
  let timer: ReturnType<typeof setInterval> | undefined;

  // The overlay shows overlay-level nags and the one running break (the app
  // demotes other breaks to toast level); the toast shows toast-level nags.
  const shown = $derived(
    nags.filter((n) =>
      mode === "overlay"
        ? n.level !== "toast"
        : n.level === "toast" && n.break_until === null,
    ),
  );

  // The toast window is resized to its content, so three cards fit.
  const TOAST_MAX = 3;
  const visible = $derived(mode === "toast" ? shown.slice(0, TOAST_MAX) : shown);
  const hidden = $derived(shown.length - visible.length);

  // Progress ring geometry (overlay breaks).
  const RING = 240;
  const STROKE = 8;
  const R = (RING - STROKE) / 2;
  const C = 2 * Math.PI * R;

  /** Remaining fraction of a running break, or null when it has no length. */
  function fraction(n: NagView): number | null {
    if (n.break_until === null || !n.break_for_ms) return null;
    return Math.min(1, Math.max(0, (n.break_until - now) / n.break_for_ms));
  }

  async function act(rule_id: string, action: Action) {
    if (busy) return;
    busy = true;
    try {
      await invoke("nag_action", { ruleId: rule_id, action });
    } finally {
      busy = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (mode !== "overlay" || e.key !== "Escape") return;
    const first = shown[0];
    if (!first) return;
    e.preventDefault();
    // Skip is always allowed; during a break it ends the break early.
    act(first.rule_id, "skip");
  }

  onMount(async () => {
    nags = await invoke<NagView[]>("current_nags");
    unlisten = await listen<NagView[]>("nags-changed", (e) => (nags = e.payload));
    timer = setInterval(() => (now = Date.now()), 500);
  });

  onDestroy(() => {
    unlisten?.();
    clearInterval(timer);
  });

  $effect(() => {
    if (mode !== "toast" || stackHeight === 0) return;
    const h = Math.ceil(stackHeight);
    if (h === reported) return;
    reported = h;
    invoke("resize_toast", { height: h }).catch(() => {});
  });
</script>

<svelte:window onkeydown={onKeydown} />

{#if mode === "toast"}
  <div class="toast-stack">
    <div class="toast-content" bind:clientHeight={stackHeight}>
    {#each visible as n, i (n.rule_id)}
      <article class="toast-card" class:urgent={n.level !== "toast"} aria-live="polite">
        <header>
          <span class="t-icon">
            <Icon name={n.break_for_ms ? "coffee" : "clock"} size={15} />
          </span>
          <h1 class="truncate">{n.rule_name}</h1>
          {#if i === 0}
            <button class="btn ghost sm dash" onclick={() => invoke("open_dashboard")}>
              Open dashboard
              <Icon name="external-link" size={12} />
            </button>
          {/if}
        </header>
        <p class="t-message">{n.message}</p>
        <div class="t-actions">
          {#if n.break_for_ms}
            <button class="btn primary" disabled={busy} onclick={() => act(n.rule_id, "break")}>
              <Icon name="coffee" size={14} />
              Start {duration(n.break_for_ms)} break
            </button>
          {/if}
          <button
            class="btn ghost"
            disabled={busy || n.snoozes_left === 0}
            onclick={() => act(n.rule_id, "snooze")}
          >
            Snooze ({n.snoozes_left} left)
          </button>
          <button class="btn ghost" disabled={busy} onclick={() => act(n.rule_id, "skip")}>
            Skip
          </button>
        </div>
      </article>
    {/each}
    {#if hidden > 0}
      <p class="more muted small">+{hidden} more</p>
    {/if}
    </div>
  </div>
{:else if shown.length > 0}
  <div class="overlay">
    {#each shown as n (n.rule_id)}
      {@const f = fraction(n)}
      <section class="o-nag" aria-live="polite">
        {#if n.break_until !== null}
          <p class="o-rule">{n.rule_name}</p>
          <div class="o-timer" class:ring={f !== null}>
            {#if f !== null}
              <svg width={RING} height={RING} viewBox="0 0 {RING} {RING}" aria-hidden="true">
                <circle class="track" cx={RING / 2} cy={RING / 2} r={R} stroke-width={STROKE} />
                <circle
                  class="fill"
                  cx={RING / 2}
                  cy={RING / 2}
                  r={R}
                  stroke-width={STROKE}
                  stroke-dasharray={C}
                  stroke-dashoffset={C * (1 - f)}
                  transform="rotate(-90 {RING / 2} {RING / 2})"
                />
              </svg>
            {/if}
            <span class="o-count" role="timer" aria-label="Break time left">
              {clock(n.break_until - now)}
            </span>
          </div>
          <p class="o-hint">Stand up, look away, stretch.</p>
          <div class="o-actions">
            <button class="btn ghost light" disabled={busy} onclick={() => act(n.rule_id, "skip")}>
              End break early
            </button>
          </div>
        {:else}
          <h1>{n.rule_name}</h1>
          <p class="o-message">{n.message}</p>
          <p class="o-since">Nagging for {duration(Math.max(0, now - n.since))}</p>
          <div class="o-actions">
            {#if n.break_for_ms}
              <button
                class="btn primary big"
                disabled={busy}
                onclick={() => act(n.rule_id, "break")}
              >
                <Icon name="coffee" size={18} />
                Start {duration(n.break_for_ms)} break
              </button>
            {/if}
            <button
              class="btn ghost light"
              disabled={busy || n.snoozes_left === 0}
              onclick={() => act(n.rule_id, "snooze")}
            >
              <Icon name="snooze" size={16} />
              Snooze ({n.snoozes_left} left)
            </button>
            <button class="btn ghost light" disabled={busy} onclick={() => act(n.rule_id, "skip")}>
              <Icon name="skip" size={16} />
              Skip
            </button>
          </div>
        {/if}
      </section>
    {/each}
    <p class="o-keys">
      Press <kbd class="o-kbd">Esc</kbd> to
      {shown[0].break_until !== null ? "end the break early" : "skip"}
    </p>
  </div>
{/if}

<style>
  :global(body.page-toast),
  :global(body.page-overlay) {
    background: transparent;
    overflow: hidden;
  }

  /* ---- Toast ---- */

  /* The window is transparent; the padding leaves room for the shadow.
     Cards sit at the bottom, next to the screen corner. */
  .toast-stack {
    height: 100vh;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    overflow: hidden;
  }
  /* The padding leaves room for the shadow; the app sizes the window to
     this element's height (`resize_toast`). */
  .toast-content {
    padding: 10px 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .toast-card {
    position: relative;
    flex: none;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: var(--shadow-lg);
    padding: 12px 14px 12px 18px;
    overflow: hidden;
    animation: toast-in 180ms ease-out both;
  }
  .toast-card::before {
    content: "";
    position: absolute;
    inset: 0 auto 0 0;
    width: 4px;
    background: var(--accent);
  }
  .toast-card.urgent::before {
    background: var(--danger);
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 24px;
  }
  .t-icon {
    display: grid;
    place-items: center;
    flex: none;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-sm);
    background: var(--accent-soft);
    color: var(--accent-fg);
  }
  .urgent .t-icon {
    background: var(--danger-soft);
    color: var(--danger);
  }
  header h1 {
    flex: 1;
    min-width: 0;
    font-size: 14.5px;
  }
  .dash {
    flex: none;
    margin-right: -6px;
    color: var(--muted);
  }

  .t-message {
    margin: 6px 0 10px;
    color: var(--text-2);
    font-size: 13px;
    line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .t-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .t-actions .btn {
    font-size: 13px;
    padding: 4px 9px;
    gap: 5px;
  }
  .t-actions .btn.primary {
    margin-right: 4px;
  }

  .more {
    flex: none;
    margin: 0;
    text-align: right;
    padding-right: 4px;
  }

  /* ---- Overlay (always dark) ---- */

  .overlay {
    --ring-fill: #a5b4fc;
    --o-text: #eef0ff;
    --o-text-2: #d6d9f0;
    --o-muted: #a3a8c8;
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 48px;
    padding: 32px;
    background: radial-gradient(ellipse at center, rgb(30 27 75 / 0.92), rgb(10 12 24 / 0.96));
    color: var(--o-text);
    color-scheme: dark;
    text-align: center;
    overflow: auto;
  }

  .o-nag {
    display: flex;
    flex-direction: column;
    align-items: center;
    animation: overlay-in 240ms ease-out both;
  }
  @keyframes overlay-in {
    from {
      opacity: 0;
      transform: scale(0.98);
    }
    to {
      opacity: 1;
      transform: none;
    }
  }

  .o-nag h1 {
    font-size: 36px;
    font-weight: 650;
    letter-spacing: -0.02em;
    margin: 0 0 12px;
  }
  .o-message {
    font-size: 20px;
    line-height: 1.45;
    max-width: 640px;
    margin: 0 0 12px;
    color: var(--o-text-2);
  }
  .o-since {
    color: var(--o-muted);
    font-size: 14px;
    margin: 0 0 28px;
    font-variant-numeric: tabular-nums;
  }

  .o-rule {
    font-size: 18px;
    font-weight: 600;
    color: var(--o-text-2);
    margin: 0 0 20px;
  }
  .o-timer {
    position: relative;
    display: grid;
    place-items: center;
  }
  .o-timer.ring {
    width: 240px;
    height: 240px;
  }
  .o-timer svg {
    position: absolute;
    inset: 0;
  }
  .track {
    fill: none;
    stroke: rgb(255 255 255 / 0.15);
  }
  .fill {
    fill: none;
    stroke: var(--ring-fill);
    stroke-linecap: round;
    transition: stroke-dashoffset 0.5s linear;
  }
  .o-count {
    position: relative;
    font-size: 96px;
    font-weight: 700;
    line-height: 1;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }
  .ring .o-count {
    font-size: 72px;
  }
  .o-hint {
    font-size: 18px;
    color: var(--o-text-2);
    margin: 24px 0;
  }

  .o-actions {
    display: flex;
    gap: 12px;
    justify-content: center;
    flex-wrap: wrap;
  }
  .o-actions .btn {
    font-size: 15px;
    padding: 10px 18px;
    border-radius: var(--radius);
  }
  .o-actions .btn.big {
    font-size: 16px;
    padding: 12px 22px;
    background: var(--ring-fill);
    border-color: var(--ring-fill);
    color: #14123a;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
  }
  .o-actions .btn.big:hover:not(:disabled) {
    background: #c7d2fe;
    border-color: #c7d2fe;
  }
  .btn.ghost.light {
    background: transparent;
    border: 1px solid rgb(255 255 255 / 0.32);
    color: var(--o-text);
  }
  .btn.ghost.light:hover:not(:disabled) {
    background: rgb(255 255 255 / 0.1);
    border-color: rgb(255 255 255 / 0.5);
    color: #fff;
  }
  .overlay .btn:focus-visible {
    box-shadow: 0 0 0 3px rgb(165 180 252 / 0.55);
  }

  .o-keys {
    margin: 0;
    font-size: 13px;
    color: var(--o-muted);
  }
  .o-kbd {
    display: inline-block;
    font-family: var(--mono);
    font-size: 11.5px;
    line-height: 18px;
    padding: 0 6px;
    border: 1px solid rgb(255 255 255 / 0.3);
    border-bottom-width: 2px;
    border-radius: 4px;
    color: var(--o-text);
  }

  @media (prefers-reduced-motion: reduce) {
    .toast-card,
    .o-nag {
      animation: none;
    }
    .fill {
      transition: none;
    }
  }
</style>
