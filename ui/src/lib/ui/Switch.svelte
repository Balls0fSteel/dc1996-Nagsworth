<script lang="ts">
  let {
    checked,
    label,
    disabled = false,
    title,
    onchange,
  }: {
    checked: boolean;
    label?: string;
    disabled?: boolean;
    title?: string;
    onchange?: (checked: boolean) => void;
  } = $props();
</script>

<label class="switch" {title}>
  <input
    type="checkbox"
    role="switch"
    aria-checked={checked}
    aria-label={label ? undefined : title}
    {checked}
    {disabled}
    onchange={(e) => onchange?.(e.currentTarget.checked)}
  />
  {#if label}<span>{label}</span>{/if}
</label>
