<script lang="ts">
  // 识别文字时截图上的检测框（需求 3.8、F6）：段落框 + 行框；选中的行填蓝色、没选的不填，
  // 左键点框切换，指针下的框边框加粗，提示可以点。
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
    hover = -1,
  }: {
    /** 识别时的选区（截图物理像素） */
    region: Rect;
    scale: number;
    result: OcrResult;
    byLine: boolean;
    /** 选中的行号 */
    pick: number[];
    /** 指针下的行（-1 表示没有） */
    hover?: number;
  } = $props();

  const keep = $derived(new Set(pick));
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
    <rect
      class="line"
      class:on={keep.has(i)}
      class:hover={i === hover}
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
    stroke: color-mix(in srgb, var(--accent) 50%, transparent);
    stroke-width: 1;
  }
  .line.on {
    fill: color-mix(in srgb, var(--accent) 22%, transparent);
    stroke: var(--accent);
  }
  .line.hover {
    stroke: var(--accent);
    stroke-width: 2;
  }
  /* 段落框只描边，免得没选中的行也泛蓝 */
  .para {
    fill: none;
    stroke: color-mix(in srgb, var(--accent) 70%, transparent);
    stroke-width: 1;
  }
</style>
