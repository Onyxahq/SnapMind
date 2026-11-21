<script lang="ts">
  // 设置页的分组卡片：无投影，只有 1px 描边环。里面一般放若干 Row。
  import type { Snippet } from 'svelte';
  let { title, children }: { title?: string; children: Snippet } = $props();
</script>

<section class="group">
  {#if title}<h3 class="sec">{title}</h3>{/if}
  <div class="card">{@render children()}</div>
</section>

<style>
  .group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .sec {
    margin: 8px 0 0;
    font-size: 14px;
    line-height: 20px;
    font-weight: 600;
  }
  .card {
    background: var(--card);
    border-radius: var(--r-panel);
    box-shadow: var(--card-ring);
  }
</style>
