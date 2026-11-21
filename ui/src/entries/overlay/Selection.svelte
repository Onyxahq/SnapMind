<script lang="ts">
  // 选区的外观：强调色描边、8 个圆形控制点、左上角的坐标与尺寸标签。
  // 坐标都是 CSS 像素；尺寸标签显示物理像素（截图里的真实大小）。
  import type { Rect } from '$lib/geometry';
  import { HANDLES, handlePoint, labelTop } from '$lib/selection';

  let {
    rect,
    label,
    handles = true,
  }: { rect: Rect; label: string; handles?: boolean } = $props();
</script>

<div
  class="sel"
  style:left="{rect.x}px"
  style:top="{rect.y}px"
  style:width="{rect.width}px"
  style:height="{rect.height}px"
>
  {#if handles}
    {#each HANDLES as h (h)}
      {@const p = handlePoint({ x: 0, y: 0, width: rect.width, height: rect.height }, h)}
      <span class="hd" style:left="{p.x}px" style:top="{p.y}px"></span>
    {/each}
  {/if}
</div>
<span class="size" style:left="{rect.x}px" style:top="{labelTop(rect.y)}px">{label}</span>

<style>
  .sel {
    position: absolute;
    /* 描边画在选区外侧，不遮住被截的内容 */
    box-shadow: 0 0 0 1.5px var(--accent);
    pointer-events: none;
  }
  .hd {
    position: absolute;
    width: 9px;
    height: 9px;
    margin: -4.5px 0 0 -4.5px;
    border-radius: 50%;
    background: var(--solid);
    box-shadow: 0 0 0 1.5px var(--accent), 0 1px 3px rgba(0, 0, 0, .25);
  }
  .size {
    position: absolute;
    font-size: 12px;
    line-height: 18px;
    padding: 3px 8px;
    border-radius: var(--r-tag);
    background: var(--chip);
    color: var(--chip-text);
    pointer-events: none;
    white-space: pre; /* 保留坐标和尺寸之间的空格 */
    font-variant-numeric: tabular-nums;
  }
</style>
