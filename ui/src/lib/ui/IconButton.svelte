<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import Icon, { type IconName } from './Icon.svelte';

  type Props = HTMLButtonAttributes & {
    icon: IconName;
    /** 必填：读屏文字，也作为悬停提示 */
    label: string;
    /** 选中态（如当前标注工具） */
    on?: boolean;
    /** 边长：工具条 30，设置页 28 */
    size?: number;
  };
  let { icon, label, on = false, size = 30, type = 'button', ...rest }: Props = $props();
</script>

<button
  {type}
  class="ib"
  class:on
  aria-label={label}
  aria-pressed={on}
  title={label}
  style:--s="{size}px"
  {...rest}
>
  <Icon name={icon} />
</button>

<style>
  .ib {
    width: var(--s);
    height: var(--s);
    border: 0;
    padding: 0;
    border-radius: var(--r-ctl);
    background: transparent;
    color: var(--text2);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    flex: none;
    transition:
      background-color var(--d-hover) var(--ease-hover),
      color var(--d-hover) var(--ease-hover),
      transform var(--d-press) var(--ease-press);
  }
  .ib:hover { background: var(--hover); color: var(--text); }
  .ib:active { transform: scale(.9); }
  .ib:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--accent); }
  .ib.on { background: var(--accent-soft); color: var(--accent); }
  .ib:disabled { opacity: .3; cursor: default; transform: none; }
</style>
