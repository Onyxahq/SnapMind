<script lang="ts">
  // 标注层：一块铺满窗口的画布，把所有标注（和正在画的那一笔）画上去，裁到选区内。
  import type { MonitorGeom, Rect } from '$lib/geometry';
  import { drawShape, type Shape } from '$lib/annotate';

  let {
    shapes,
    draft,
    clip,
    mon,
  }: {
    shapes: Shape[];
    draft: Shape | null;
    /** 选区（本屏物理像素），标注只显示在选区内 */
    clip: Rect;
    mon: MonitorGeom;
  } = $props();

  let canvas: HTMLCanvasElement;
  let frame = 0;

  function paint() {
    frame = 0;
    if (!canvas) return;
    const dpr = devicePixelRatio || 1;
    const w = Math.round(innerWidth * dpr);
    const h = Math.round(innerHeight * dpr);
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
    const g = canvas.getContext('2d')!;
    g.clearRect(0, 0, w, h);
    // 画布像素 / 截图物理像素
    const k = dpr / mon.scale;
    g.save();
    g.beginPath();
    g.rect(clip.x * k, clip.y * k, clip.width * k, clip.height * k);
    g.clip();
    for (const s of shapes) drawShape(g, s, k);
    if (draft) drawShape(g, draft, k);
    g.restore();
  }

  // 同一帧里的多次变化只画一次
  $effect(() => {
    void shapes;
    void draft;
    void clip.x;
    void clip.y;
    void clip.width;
    void clip.height;
    if (!frame) frame = requestAnimationFrame(paint);
  });
</script>

<canvas bind:this={canvas} aria-hidden="true"></canvas>

<style>
  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
</style>
