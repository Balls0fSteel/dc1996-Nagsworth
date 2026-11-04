<script lang="ts" generics="T extends string | number">
  let {
    options,
    value,
    onchange,
    label,
  }: {
    options: readonly { id: T; label: string }[];
    value: T;
    onchange: (id: T) => void;
    /** Accessible name of the group. */
    label?: string;
  } = $props();

  function key(e: KeyboardEvent, i: number) {
    const step = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = options[(i + step + options.length) % options.length];
    onchange(next.id);
    const group = (e.currentTarget as HTMLElement).parentElement;
    queueMicrotask(() =>
      group?.querySelector<HTMLButtonElement>('[aria-checked="true"]')?.focus(),
    );
  }
</script>

<div class="segmented" role="radiogroup" aria-label={label}>
  {#each options as o, i (o.id)}
    <button
      type="button"
      role="radio"
      aria-checked={o.id === value}
      tabindex={o.id === value ? 0 : -1}
      class:on={o.id === value}
      onclick={() => onchange(o.id)}
      onkeydown={(e) => key(e, i)}
    >
      {o.label}
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    padding: 2px;
    gap: 2px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--muted) 14%, transparent);
    border: 1px solid var(--border);
  }
  button {
    border: none;
    box-shadow: none;
    background: transparent;
    color: var(--text-2);
    padding: 2px 10px;
    font-size: 12.5px;
    border-radius: 4px;
  }
  button:hover:not(:disabled) {
    background: color-mix(in srgb, var(--panel) 50%, transparent);
    color: var(--text);
  }
  button.on,
  button.on:hover:not(:disabled) {
    background: var(--panel);
    color: var(--text);
    box-shadow: var(--shadow-sm);
  }
  button:focus-visible {
    box-shadow: var(--focus);
  }
</style>
