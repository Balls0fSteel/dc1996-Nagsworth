<script lang="ts">
  import { fly } from "svelte/transition";
  import { notify } from "../notify.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each notify.list as n (n.id)}
    <div class="toast {n.kind}" transition:fly={{ y: 12, duration: 180 }}>
      <span class="ic"><Icon name={n.kind === "ok" ? "check" : "alert"} size={16} /></span>
      <span class="text">{n.text}</span>
      <button class="btn ghost icon sm" aria-label="Dismiss" onclick={() => notify.dismiss(n.id)}>
        <Icon name="x" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 900;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 8px;
    pointer-events: none;
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 8px;
    max-width: 380px;
    padding: 8px 8px 8px 12px;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--border);
    border-left: 3px solid var(--ok);
    border-radius: var(--radius);
    box-shadow: var(--shadow-lg);
  }
  .toast.err {
    border-left-color: var(--danger);
  }
  .ic {
    display: grid;
    place-items: center;
    color: var(--ok);
  }
  .toast.err .ic {
    color: var(--danger);
  }
  .text {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
</style>
