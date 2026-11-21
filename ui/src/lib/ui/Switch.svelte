<script lang="ts">
  let {
    checked = $bindable(false),
    disabled = false,
    label,
    onchange,
  }: { checked?: boolean; disabled?: boolean; label: string; onchange?: (v: boolean) => void } = $props();

  function toggle() {
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  class="sw"
  class:on={checked}
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={toggle}
></button>

<style>
  .sw {
    width: 40px;
    height: 22px;
    border-radius: var(--r-full);
    background: var(--line2);
    position: relative;
    border: 0;
    padding: 0;
    cursor: pointer;
    flex: none;
    transition: background-color 200ms var(--ease-hover);
  }
  .sw::after {
    content: '';
    position: absolute;
    left: 3px;
    top: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff; /* 滑块在两种模式下都是白色（规范） */
    box-shadow: 0 1px 3px rgba(0, 0, 0, .25);
    transition: transform var(--d-switch) var(--ease-switch), width var(--d-press) var(--ease-press);
  }
  /* 按住时滑块稍微拉长，松手回弹，手感更跟手 */
  .sw:active:not(:disabled)::after { width: 20px; }
  .sw.on:active:not(:disabled)::after { transform: translateX(14px); }
  .sw.on { background: var(--accent); }
  .sw.on::after { transform: translateX(18px); }
  .sw:focus-visible { outline: none; box-shadow: 0 0 0 3px var(--accent-soft); }
  .sw:disabled { opacity: .4; cursor: not-allowed; }
</style>
