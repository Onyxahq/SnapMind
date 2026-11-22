<script lang="ts">
  // 识别文字时截图上的检测框（需求 3.8）：段落框 + 行框；右键划选后选中的行加深、没选的变淡。
  // SVG 盖在选区上，viewBox 用选区的截图像素，行框坐标直接用 Rust 返回的值；描边按 CSS 像素画。
  import type { Rect } from '$lib/geometry';
  import type { OcrResult } from '$lib/ipc';
  import { paragraphBoxes } from '$lib/ocr';

  let {
    region,
    scale,
    result,
    byLine,
    pick,
    drag,
  }: {
    /** 识别时的选区（截图物理像素） */
    region: Rect;
    scale: number;
    result: OcrResult;
    byLine: boolean;
    pick: number[] | null;
    /** 正在右键划的矩形（相对选区，截图像素） */
    drag: Rect | null;
  } = $props();

  const keep = $derived(pick ? new Set(pick) : null);
  const paras = $derived(byLine ? [] : paragraphBoxes(result, keep));
  /** 框比文字外扩一点，圆角跟着字号走 */
  const pad = $derived(2 * scale);
  const r = $derived(3 * scale);
</script>

<svg
  class="boxes"
  style:left="{region.x / scale}px"
  style:top="{region.y / scale}px"
  style:width="{region.width / scale}px"
  style:height="{region.height / scale}px"
  viewBox="0 0 {region.width} {region.height}"
  aria-hidden="true"
>
  {#each result.lines as l, i (i)}
    {@const on = !keep || keep.has(i)}
    <rect
      class="line"
      class:off={!on}
      class:strong={byLine || !!keep}
      x={l.x - pad / 2}
      y={l.y - pad / 2}
      width={l.width + pad}
      height={l.height + pad}
      rx={r}
      vector-effect="non-scaling-stroke"
    />
  {/each}
  {#each paras as p, i (i)}
    <rect
      class="para"
      x={p.x - pad * 2}
      y={p.y - pad * 2}
      width={p.width + pad * 4}
      height={p.height + pad * 4}
      rx={r * 1.6}
      vector-effect="non-scaling-stroke"
    />
  {/each}
  {#if drag}
    <rect class="drag" x={drag.x} y={drag.y} width={drag.width} height={drag.height} vector-effect="non-scaling-stroke" />
  {/if}
</svg>

<style>
  .boxes {
    position: absolute;
    z-index: 5;
    pointer-events: none;
    overflow: visible;
    animation: boxes-in var(--d-in) var(--ease-in) both;
  }
  @keyframes boxes-in {
    from { opacity: 0; }
  }
  rect {
    transition:
      fill var(--d-hover) var(--ease-hover),
      stroke var(--d-hover) var(--ease-hover),
      opacity var(--d-hover) var(--ease-hover);
  }
  .line {
    fill: transparent;
    stroke: color-mix(in srgb, var(--accent) 45%, transparent);
    stroke-width: 1;
  }
  .line.strong {
    fill: color-mix(in srgb, var(--accent) 10%, transparent);
    stroke: var(--accent);
    stroke-width: 1.25;
  }
  /* 没选中的行：盖一层遮罩让文字变淡，不描边 */
  .line.off {
    fill: var(--scrim);
    stroke: transparent;
  }
  .para {
    fill: color-mix(in srgb, var(--accent) 6%, transparent);
    stroke: var(--accent);
    stroke-width: 1.5;
  }
  .drag {
    fill: color-mix(in srgb, var(--accent) 12%, transparent);
    stroke: var(--accent);
    stroke-width: 1.25;
    stroke-dasharray: 5 4;
    transition: none;
  }
</style>
