<script lang="ts">
  // The toast and overlay windows. The app opens and closes them to match the
  // service's nag board; this page reads the nags from the API
  // (`/nags/current` and the event stream) and sends the user's choice back
  // as `/control/*` requests.
  import { onDestroy, onMount } from "svelte";
  import { invoke, isTauri } from "@tauri-apps/api/core";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { clock, duration } from "./lib/format";
  import { api, nagsCurrent, subscribeEvents, type NagView, type NagsView } from "./lib/api";
  import Icon from "./lib/ui/Icon.svelte";

  type Action = "snooze" | "skip" | "break";

  let { mode }: { mode: "toast" | "overlay" } = $props();

  let nags = $state<NagView[]>([]);
  let now = $state(Date.now());
  let busy = $state(false);
  // Content height of the toast stack; the app sizes the window to it.
  let stackHeight = $state(0);
  let reported = 0;
  let unsubscribe: (() => void) | undefined;
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

  // ---- Routine steps ----
  // Steps run on their own seconds from the break's start; "Next" (or Space)
  // jumps ahead and the schedule continues from that step at that moment.
  // The engine's break timer stays the truth; this is only a view on it.
  interface StepBase {
    index: number;
    at: number;
  }
  let stepBase = $state<Map<string, StepBase>>(new Map());

  interface StepView {
    index: number;
    /** Milliseconds left in the current step. */
    left: number;
  }

  function stepView(n: NagView): StepView | null {
    const r = n.routine;
    if (!r || r.steps.length === 0 || n.break_until === null || !n.break_for_ms) return null;
    const base = stepBase.get(n.rule_id) ?? { index: 0, at: n.break_until - n.break_for_ms };
    let t = base.at;
    for (let i = base.index; i < r.steps.length; i++) {
      const end = t + r.steps[i].seconds * 1000;
      if (now < end || i === r.steps.length - 1) {
        return { index: i, left: Math.max(0, end - now) };
      }
      t = end;
    }
    return { index: r.steps.length - 1, left: 0 };
  }

  function nextStep(n: NagView) {
    const v = stepView(n);
    if (!v || !n.routine || v.index >= n.routine.steps.length - 1) return;
    const m = new Map(stepBase);
    m.set(n.rule_id, { index: v.index + 1, at: now });
    stepBase = m;
  }

  // ---- Step-change tick (opt-in, `[exercises] step_sound`) ----
  // Only the first overlay window ticks, so several monitors do not echo.
  let stepSound = false;
  let audio: AudioContext | null = null;
  let lastStep: string | null = null;
  // The page's mode never changes, so reading the label once is right.
  const ticks = isTauri() && getCurrentWebviewWindow().label === "overlay-0";

  async function loadPrefs() {
    if (!ticks) return;
    try {
      stepSound = (await api.exercises()).config.step_sound;
    } catch {
      stepSound = false;
    }
  }

  function tick() {
    try {
      audio ??= new AudioContext();
      // WebView autoplay rules may start the context suspended.
      if (audio.state === "suspended") void audio.resume();
      const t = audio.currentTime;
      const osc = audio.createOscillator();
      const gain = audio.createGain();
      osc.type = "sine";
      osc.frequency.value = 880;
      gain.gain.setValueAtTime(0.0001, t);
      gain.gain.exponentialRampToValueAtTime(0.08, t + 0.01);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.15);
      osc.connect(gain).connect(audio.destination);
      osc.start(t);
      osc.stop(t + 0.16);
    } catch {
      // No audio device; the tick is a nicety.
    }
  }

  $effect(() => {
    if (!ticks) return;
    const first = shown[0];
    const sv = first ? stepView(first) : null;
    const key = first && sv ? `${first.rule_id}:${first.break_until}:${sv.index}` : null;
    // A new step of the same break ticks; the first step of a break does not.
    if (key && lastStep && stepSound && sv && sv.index > 0 && key !== lastStep) {
      const [rule, until] = lastStep.split(":");
      if (rule === first.rule_id && until === String(first.break_until)) tick();
    }
    lastStep = key;
  });

  async function act(rule_id: string, action: Action) {
    if (busy) return;
    busy = true;
    try {
      if (action === "snooze") await api.snooze(rule_id);
      else if (action === "skip") await api.skip(rule_id);
      else await api.startBreak(rule_id);
    } finally {
      busy = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (mode !== "overlay") return;
    const first = shown[0];
    if (!first) return;
    if (e.key === " " && first.break_until !== null && first.routine) {
      e.preventDefault();
      nextStep(first);
      return;
    }
    if (e.key !== "Escape") return;
    e.preventDefault();
    // Skip is always allowed; during a break it ends the break early.
    act(first.rule_id, "skip");
  }

  onMount(async () => {
    const reload = async () => {
      try {
        nags = (await nagsCurrent()).nags;
      } catch {
        // The stream reconnects and reloads.
      }
    };
    await reload();
    await loadPrefs();
    unsubscribe = subscribeEvents(
      ["nags"],
      (e) => {
        if (e.source !== "nags") return;
        nags = (e.event as NagsView).nags;
        void loadPrefs();
      },
      () => void reload(),
    );
    timer = setInterval(() => (now = Date.now()), 500);
  });

  onDestroy(() => {
    unsubscribe?.();
    clearInterval(timer);
    void audio?.close();
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
        {#if n.routine && n.routine.steps.length > 0}
          {@const first = n.routine.steps[0]}
          <p class="t-cue">
            <span class="t-cue-name">{n.routine.steps.length > 1 ? "First" : "Try"}: {first.name}</span>
            <span class="muted">· {first.reps}</span>
            <span class="t-cue-text">{first.cue}</span>
          </p>
        {/if}
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
      {@const sv = stepView(n)}
      <section class="o-nag" class:with-routine={sv !== null} aria-live="polite">
        {#if n.break_until !== null}
          <p class="o-rule">{n.rule_name}</p>
          {#if n.routine?.note}
            <p class="o-note">{n.routine.note}</p>
          {/if}
          <div class="o-break">
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
          {#if sv && n.routine}
            {@const cur = n.routine.steps[sv.index]}
            <div class="o-routine">
              <div class="o-step">
                <p class="o-step-head">
                  <span class="o-step-n">{sv.index + 1} / {n.routine.steps.length}</span>
                  <span class="o-step-name">{cur.name}</span>
                  <span class="o-step-reps">{cur.reps}</span>
                </p>
                <ol class="o-lines">
                  {#each cur.lines as line, i (i)}
                    <li>{line}</li>
                  {/each}
                </ol>
                <p class="o-step-left" role="timer" aria-label="Step time left">
                  {clock(sv.left)}
                  {#if sv.index < n.routine.steps.length - 1}
                    <button class="btn ghost light sm" onclick={() => nextStep(n)}>
                      Next
                      <Icon name="skip" size={14} />
                    </button>
                  {/if}
                </p>
              </div>
              <ol class="o-list" aria-label="Routine">
                {#each n.routine.steps as s, i (s.id)}
                  <li class:done={i < sv.index} class:current={i === sv.index}>
                    <span class="o-list-name">{s.name}</span>
                    <span class="o-list-secs">{clock(s.seconds * 1000)}</span>
                  </li>
                {/each}
              </ol>
            </div>
          {/if}
          </div>
          {#if !sv}
            <p class="o-hint">Stand up, look away, stretch.</p>
          {:else}
            <p class="o-hint small">Not medical advice. Stop anything that hurts.</p>
          {/if}
          <div class="o-actions">
            <button class="btn ghost light" disabled={busy} onclick={() => act(n.rule_id, "skip")}>
              End break early
            </button>
          </div>
        {:else}
          <h1>{n.rule_name}</h1>
          <p class="o-message">{n.message}</p>
          {#if n.routine && n.routine.steps.length > 0}
            <p class="o-preview">
              {n.routine.steps.length > 1 ? "Planned" : "Suggested"}:
              {n.routine.steps.map((s) => s.name).join(", ")}
            </p>
          {/if}
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
      {#if shown[0].break_until !== null && shown[0].routine}
        <kbd class="o-kbd">Space</kbd> next step ·
      {/if}
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

  .t-cue {
    margin: -4px 0 10px;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    background: var(--accent-soft);
    font-size: 12.5px;
    line-height: 1.4;
    display: flex;
    flex-wrap: wrap;
    gap: 0 6px;
  }
  .t-cue-name {
    font-weight: 600;
    color: var(--accent-fg);
  }
  .t-cue-text {
    flex-basis: 100%;
    color: var(--text-2);
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
  .o-hint.small {
    font-size: 13px;
    color: var(--o-muted);
    margin: 16px 0;
  }
  .o-note {
    margin: -8px 0 20px;
    font-size: 15px;
    color: var(--o-muted);
  }
  .o-preview {
    margin: 0 0 12px;
    font-size: 15px;
    color: var(--o-muted);
    max-width: 640px;
  }

  /* ---- Routine ---- */
  .o-break {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 56px;
    flex-wrap: wrap;
  }
  .o-routine {
    display: flex;
    gap: 40px;
    align-items: flex-start;
    text-align: left;
    max-width: 720px;
  }
  .o-step {
    flex: 1 1 360px;
    min-width: 0;
  }
  .o-step-head {
    margin: 0 0 10px;
    display: flex;
    align-items: baseline;
    gap: 10px;
    flex-wrap: wrap;
  }
  .o-step-n {
    font-size: 13px;
    color: var(--o-muted);
    font-variant-numeric: tabular-nums;
  }
  .o-step-name {
    font-size: 28px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  .o-step-reps {
    font-size: 15px;
    color: var(--o-text-2);
  }
  .o-lines {
    margin: 0;
    padding-left: 22px;
    font-size: 17px;
    line-height: 1.5;
    color: var(--o-text-2);
  }
  .o-lines li + li {
    margin-top: 4px;
  }
  .o-step-left {
    margin: 14px 0 0;
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 24px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .o-step-left .btn.sm {
    font-size: 13px;
    padding: 4px 10px;
  }
  .o-list {
    flex: 0 0 200px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 14px;
    color: var(--o-muted);
  }
  .o-list li {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 5px 8px;
    border-left: 2px solid rgb(255 255 255 / 0.12);
  }
  .o-list li.done {
    text-decoration: line-through;
    opacity: 0.6;
  }
  .o-list li.current {
    color: var(--o-text);
    border-left-color: var(--ring-fill);
    font-weight: 600;
  }
  .o-list-secs {
    font-variant-numeric: tabular-nums;
  }
  @media (max-width: 900px) {
    .o-routine {
      flex-direction: column;
      gap: 16px;
    }
    .o-list {
      flex-basis: auto;
    }
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
