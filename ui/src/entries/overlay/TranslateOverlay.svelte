<script lang="ts">
  // 原位覆盖（需求第 4 节、M6.2）：译文按段落画在原文位置上。画布按截图物理像素绘制，
  // 和复制 / 贴图时导出的图片是同一套代码画的。
  import type { Rect } from '$lib/geometry';
  import { drawOverlay, type Fitted, type OverlayBlock } from '$lib/overlay';

  let { region, scale, blocks, fits }: { region: Rect; scale: number; blocks: OverlayBlock[]; fits: Fitted[] } =
    $props();

  let canvas = $state<HTMLCanvasElement>();
  $effect(() => {
    if (!canvas) return;
    canvas.width = Math.max(1, region.width);
    canvas.height = Math.max(1, region.height);
    const g = canvas.getContext('2d')!;
    g.clearRect(0, 0, canvas.width, canvas.height);
    drawOverlay(g, blocks, fits);
  });
</script>

<canvas
  bind:this={canvas}
  class="overlay"
  style:left="{region.x / scale}px"
  style:top="{region.y / scale}px"
  style:width="{region.width / scale}px"
  style:height="{region.height / scale}px"
  aria-hidden="true"
></canvas>

<style>
  .overlay {
    position: absolute;
    z-index: 4;
    pointer-events: none;
    animation: overlay-in 260ms var(--ease-in) both;
  }
  @keyframes overlay-in {
    from { opacity: 0; }
  }
</style>
