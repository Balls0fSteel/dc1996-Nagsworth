<script lang="ts">
  let {
    label,
    value,
    sub,
    color,
  }: {
    label: string;
    value: string;
    sub?: string;
    /** CSS color for the left stripe and label dot, e.g. `var(--cat-work)`. */
    color?: string;
  } = $props();
</script>

<div class="tile" style:--tile-color={color ?? "var(--border-strong)"} class:colored={!!color}>
  <div class="label">
    {#if color}<span class="dot"></span>{/if}
    {label}
  </div>
  <div class="value num">{value}</div>
  {#if sub}<div class="sub">{sub}</div>{/if}
</div>

<style>
  .tile {
    position: relative;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-sm);
    padding: 12px 14px;
    min-width: 0;
    overflow: hidden;
  }
  .tile.colored::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--tile-color);
  }
  .label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--tile-color);
    flex: none;
  }
  .value {
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.02em;
    margin-top: 2px;
    white-space: nowrap;
  }
  .sub {
    font-size: 12px;
    color: var(--muted);
    margin-top: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
