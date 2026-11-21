<script lang="ts">
  // 卡片里的一行设置：左边标题和说明，右边控件。相邻两行之间有分割线。
  import type { Snippet } from 'svelte';
  let { title, desc, children }: { title: string; desc?: string; children?: Snippet } = $props();
</script>

<div class="row">
  <div class="rt">
    <b>{title}</b>
    {#if desc}<span>{desc}</span>{/if}
  </div>
  {@render children?.()}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 16px;
    min-height: 34px;
  }
  .row:not(:first-child) { border-top: 1px solid var(--line); }
  .rt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }
  .rt b { font-size: 14px; font-weight: 500; line-height: 20px; }
  .rt span { font-size: 12px; line-height: 18px; color: var(--text2); }
</style>
