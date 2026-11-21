<script lang="ts" generics="T extends string">
  import type { HTMLSelectAttributes } from 'svelte/elements';
  import Icon from './Icon.svelte';

  type Props = Omit<HTMLSelectAttributes, 'value'> & {
    options: { value: T; label: string }[];
    value: T;
    width?: string;
  };
  let { options, value = $bindable(), width, ...rest }: Props = $props();
</script>

<span class="sel" style:width>
  <select class="field" bind:value {...rest}>
    {#each options as o (o.value)}
      <option value={o.value}>{o.label}</option>
    {/each}
  </select>
  <span class="chev"><Icon name="chevronDown" size={14} stroke={2} /></span>
</span>

<style>
  .sel {
    position: relative;
    display: inline-flex;
    min-width: 0;
  }
  .field {
    appearance: none;
    width: 100%;
    height: 34px;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line2);
    background: var(--field);
    color: var(--text);
    font-size: 13px;
    padding: 0 30px 0 10px;
    cursor: pointer;
    transition:
      border-color var(--d-hover) var(--ease-hover),
      box-shadow var(--d-hover) var(--ease-hover);
  }
  .field:hover:not(:focus) { border-color: var(--text3); }
  .field:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  /* 下拉列表由系统绘制，跟随 color-scheme 自动适配明暗 */
  .field option { background: var(--solid); color: var(--text); }
  .chev {
    position: absolute;
    right: 9px;
    top: 50%;
    transform: translateY(-50%);
    color: var(--text3);
    pointer-events: none;
    display: inline-flex;
  }
</style>
