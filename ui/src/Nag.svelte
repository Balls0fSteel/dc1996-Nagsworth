<script lang="ts">
  // The toast and overlay windows. The app opens them when a rule nags and
  // closes them when the nag is dismissed; this page only renders the nags
  // and sends the user's choice back.
  import { onDestroy, onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { clock, duration } from "./lib/format";

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

  let { mode }: { mode: "toast" | "overlay" } = $props();

  let nags = $state<NagView[]>([]);
  let now = $state(Date.now());
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

  async function act(rule_id: string, action: "snooze" | "skip" | "break") {
    await invoke("nag_action", { ruleId: rule_id, action });
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
</script>

<div class="nag {mode}">
  {#each shown as n (n.rule_id)}
    <article>
      <h1>{n.rule_name}</h1>
      {#if n.break_until !== null}
        <p class="countdown">{clock(n.break_until - now)}</p>
        <p class="message">Break in progress. Stand up, look away, stretch.</p>
        <div class="actions">
          <button onclick={() => act(n.rule_id, "skip")}>End break early</button>
        </div>
      {:else}
        <p class="message">{n.message}</p>
        {#if mode === "overlay"}
          <p class="muted">Nagging for {duration(Math.max(0, now - n.since))}</p>
        {/if}
        <div class="actions">
          {#if n.break_for_ms}
            <button class="primary" onclick={() => act(n.rule_id, "break")}>
              Start {duration(n.break_for_ms)} break
            </button>
          {/if}
          <button onclick={() => act(n.rule_id, "snooze")} disabled={n.snoozes_left === 0}>
            Snooze ({n.snoozes_left} left)
          </button>
          <button onclick={() => act(n.rule_id, "skip")}>Skip</button>
        </div>
      {/if}
    </article>
  {:else}
    <p class="muted">Nothing to nag about.</p>
  {/each}
  {#if mode === "toast"}
    <button class="link" onclick={() => invoke("open_dashboard")}>Open dashboard</button>
  {/if}
</div>

<style>
  :global(body.page-toast),
  :global(body.page-overlay) {
    background: transparent;
    overflow: hidden;
  }
  .nag.toast {
    margin: 8px;
    padding: 14px 16px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
    height: calc(100vh - 16px);
    overflow: auto;
  }
  .nag.toast h1 {
    font-size: 15px;
    margin: 0 0 4px;
  }
  .nag.toast .message {
    margin: 0 0 10px;
  }
  .nag.overlay {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 32px;
    background: rgb(10 12 18 / 0.88);
    color: #f3f4f6;
    text-align: center;
  }
  .nag.overlay h1 {
    font-size: 36px;
    margin: 0 0 12px;
  }
  .nag.overlay .message {
    font-size: 20px;
    max-width: 640px;
  }
  .nag.overlay .muted {
    color: #9ca3af;
  }
  .countdown {
    font-size: 72px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    margin: 0;
  }
  .actions {
    display: flex;
    gap: 8px;
    justify-content: center;
    flex-wrap: wrap;
  }
  .nag.toast .actions {
    justify-content: flex-start;
  }
  .nag.overlay button {
    font-size: 16px;
    padding: 10px 18px;
  }
  article + article {
    margin-top: 14px;
  }
  .link {
    border: none;
    background: none;
    color: var(--accent);
    padding: 0;
    margin-top: 10px;
  }
</style>
