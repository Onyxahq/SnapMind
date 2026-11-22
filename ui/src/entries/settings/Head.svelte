<script lang="ts">
  // 设置页的页头：大标题、说明，右边可以放一个按钮（如「恢复默认」）。
  import type { Snippet } from 'svelte';
  let { title, desc, children }: { title: string; desc?: string; children?: Snippet } = $props();
</script>

<div class="head">
  <div class="t">
    <h2>{title}</h2>
    {#if desc}<p>{desc}</p>{/if}
  </div>
  {@render children?.()}
</div>

<style>
  .head { display: flex; align-items: flex-end; gap: 12px; }
  .t { flex: 1; min-width: 0; }
  h2 { margin: 0; font-size: 26px; line-height: 36px; font-weight: 600; }
  p { margin: 2px 0 0; font-size: 13px; line-height: 20px; color: var(--text2); }
</style>
