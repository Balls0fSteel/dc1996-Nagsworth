<script lang="ts">
  import { icons, type IconName } from "../icons";

  let {
    name,
    size = 16,
    strokeWidth = 1.75,
    class: className = "",
    title,
  }: {
    name: IconName;
    size?: number;
    strokeWidth?: number;
    class?: string;
    /** When set the icon is announced; otherwise it is decorative. */
    title?: string;
  } = $props();
</script>

<svg
  class="icon {className}"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden={title ? undefined : "true"}
  role={title ? "img" : undefined}
  aria-label={title}
  focusable="false"
>
  <path d={icons[name]} />
</svg>

<style>
  .icon {
    flex: none;
    display: inline-block;
    vertical-align: middle;
  }
</style>
