<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    subtitle,
    actions,
    children,
    padded = true,
    class: className = "",
  }: {
    title?: string;
    subtitle?: string;
    /** Right-aligned controls in the header row. */
    actions?: Snippet;
    children: Snippet;
    /** `false` drops the body's side padding (for edge-to-edge tables). */
    padded?: boolean;
    class?: string;
  } = $props();
</script>

<section class="card {className}">
  {#if title || subtitle || actions}
    <header class="card-head">
      <div class="card-titles">
        {#if title}<h2>{title}</h2>{/if}
        {#if subtitle}<div class="card-sub">{subtitle}</div>{/if}
      </div>
      {#if actions}<div class="card-actions">{@render actions()}</div>{/if}
    </header>
  {/if}
  <div class="card-body" class:flush={!padded}>
    {@render children()}
  </div>
</section>
