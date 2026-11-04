<script lang="ts">
  // A fixed-position tooltip near a pointer position (client coordinates),
  // flipped and clamped so it stays inside the viewport. Usage:
  //   <Tooltip x={e.clientX} y={e.clientY} show={hovering}>...</Tooltip>
  // Keep it outside any element with `container-type` (container queries):
  // such an element becomes the containing block for `position: fixed`, and
  // the tooltip would be offset by the container's position.
  import type { Snippet } from "svelte";

  let {
    x,
    y,
    show,
    offset = 12,
    children,
  }: {
    x: number;
    y: number;
    show: boolean;
    /** Gap between the pointer and the tooltip. */
    offset?: number;
    children: Snippet;
  } = $props();

  let w = $state(0);
  let h = $state(0);

  const pos = $derived.by(() => {
    const vw = typeof window === "undefined" ? 1e4 : window.innerWidth;
    const vh = typeof window === "undefined" ? 1e4 : window.innerHeight;
    const m = 6;
    let left = x + offset;
    if (left + w > vw - m) left = x - offset - w;
    left = Math.max(m, Math.min(left, vw - w - m));
    let top = y + offset;
    if (top + h > vh - m) top = y - offset - h;
    top = Math.max(m, Math.min(top, vh - h - m));
    return { left, top };
  });
</script>

{#if show}
  <div
    class="tooltip"
    role="tooltip"
    bind:offsetWidth={w}
    bind:offsetHeight={h}
    style:left="{pos.left}px"
    style:top="{pos.top}px"
  >
    {@render children()}
  </div>
{/if}

<style>
  .tooltip {
    position: fixed;
    z-index: 1000;
    pointer-events: none;
    max-width: 320px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--border-strong);
    box-shadow: var(--shadow-lg);
    font-size: 12.5px;
    line-height: 1.4;
  }
</style>
