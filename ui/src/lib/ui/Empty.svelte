<script lang="ts">
  import type { Snippet } from "svelte";
  import type { IconName } from "../icons";
  import Icon from "./Icon.svelte";

  let {
    icon = "info",
    title,
    hint,
    children,
  }: {
    icon?: IconName;
    title: string;
    hint?: string;
    /** Optional action, e.g. a button. */
    children?: Snippet;
  } = $props();
</script>

<div class="empty">
  <div class="empty-icon"><Icon name={icon} size={18} /></div>
  <div class="empty-title">{title}</div>
  {#if hint}<div class="empty-hint">{hint}</div>{/if}
  {#if children}<div class="empty-action">{@render children()}</div>{/if}
</div>
