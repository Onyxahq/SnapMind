<script lang="ts">
  // 贴图上的标注层：铺满图片区域的画布，按当前显示尺寸把标注（图片像素坐标）画上去。
  import { drawShape, type Shape } from '$lib/annotate';

  let {
    shapes,
    draft,
    natural,
    src,
  }: {
    shapes: Shape[];
    draft: Shape | null;
    /** 图片原始尺寸（标注坐标的单位） */
    natural: { width: number; height: number };
    src: HTMLImageElement | undefined;
  } = $props();

  let canvas: HTMLCanvasElement;
  let frame = 0;
  let w = $state(0);
  let h = $state(0);

  function paint() {
    frame = 0;
    if (!canvas) return;
    const dpr = devicePixelRatio || 1;
    const cw = Math.round(w * dpr);
    const ch = Math.round(h * dpr);
    if (canvas.width !== cw) canvas.width = cw;
    if (canvas.height !== ch) canvas.height = ch;
    const g = canvas.getContext('2d')!;
    g.clearRect(0, 0, cw, ch);
    const k = cw / natural.width; // 画布像素 / 图片像素
    const img = src?.naturalWidth ? src : undefined;
    for (const s of shapes) drawShape(g, s, k, img);
    if (draft) drawShape(g, draft, k, img);
  }

  $effect(() => {
    void shapes;
    void draft;
    void w;
    void h;
    void natural.width;
    if (!frame) frame = requestAnimationFrame(paint);
  });
</script>

<canvas bind:this={canvas} bind:clientWidth={w} bind:clientHeight={h} aria-hidden="true"></canvas>

<style>
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
</style>
