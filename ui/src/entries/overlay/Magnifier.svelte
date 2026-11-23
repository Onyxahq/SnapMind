<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 放大镜：光标周围 11×7 个截图像素放大 14 倍，中间一格是当前像素；下方显示坐标和颜色值。
  // 直接从已解码的冻结帧 <img> 画到画布上，不把整张截图读成像素数组。
  import type { MonitorGeom, Point } from '$lib/geometry';
  import { formatColor, magnifierPos, type ColorFormat, type RGB } from '$lib/color';

  const COLS = 11;
  const ROWS = 7;
  const CELL = 14;
  const GRID_W = COLS * CELL; // 154
  const GRID_H = ROWS * CELL; // 98
  /** 整个放大镜的尺寸（网格 + 信息栏），用于靠边翻转 */
  const SIZE = { width: GRID_W + 2, height: GRID_H + 2 + 62 };

  let {
    img,
    mon,
    px,
    cursorCss,
    format,
    rgb = $bindable(null),
  }: {
    img: HTMLImageElement;
    mon: MonitorGeom;
    /** 光标所在的截图像素（本屏物理像素） */
    px: Point;
    /** 光标的 CSS 坐标（决定放大镜摆在哪） */
    cursorCss: Point;
    format: ColorFormat;
    /** 当前像素颜色，供父组件复制 */
    rgb?: RGB | null;
  } = $props();

  let canvas: HTMLCanvasElement;
  const probe = document.createElement('canvas');
  probe.width = probe.height = 1;
  const probeCtx = probe.getContext('2d', { willReadFrequently: true })!;

  const pos = $derived(magnifierPos(cursorCss, SIZE, { width: innerWidth, height: innerHeight }));
  const value = $derived(rgb ? formatColor(rgb, format) : '');
  const swatch = $derived(rgb ? `rgb(${rgb.r}, ${rgb.g}, ${rgb.b})` : 'transparent');

  function draw() {
    if (!canvas || !img.naturalWidth) return;
    const dpr = devicePixelRatio || 1;
    const w = Math.round(GRID_W * dpr);
    const h = Math.round(GRID_H * dpr);
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
    const g = canvas.getContext('2d')!;
    g.imageSmoothingEnabled = false;
    g.fillStyle = '#000';
    g.fillRect(0, 0, w, h);

    // 取样区域裁到截图范围内，超出部分保持黑色
    const sx = px.x - (COLS >> 1);
    const sy = px.y - (ROWS >> 1);
    const x0 = Math.max(sx, 0), y0 = Math.max(sy, 0);
    const x1 = Math.min(sx + COLS, img.naturalWidth), y1 = Math.min(sy + ROWS, img.naturalHeight);
    const cw = w / COLS, ch = h / ROWS;
    if (x1 > x0 && y1 > y0) {
      g.drawImage(img, x0, y0, x1 - x0, y1 - y0, (x0 - sx) * cw, (y0 - sy) * ch, (x1 - x0) * cw, (y1 - y0) * ch);
    }

    // 像素网格
    g.strokeStyle = 'rgba(127,127,127,.22)';
    g.lineWidth = 1;
    g.beginPath();
    for (let i = 1; i < COLS; i++) {
      const x = Math.round(i * cw) + 0.5;
      g.moveTo(x, 0);
      g.lineTo(x, h);
    }
    for (let j = 1; j < ROWS; j++) {
      const y = Math.round(j * ch) + 0.5;
      g.moveTo(0, y);
      g.lineTo(w, y);
    }
    g.stroke();

    // 当前像素的颜色
    if (px.x >= 0 && px.y >= 0 && px.x < img.naturalWidth && px.y < img.naturalHeight) {
      probeCtx.drawImage(img, px.x, px.y, 1, 1, 0, 0, 1, 1);
      const d = probeCtx.getImageData(0, 0, 1, 1).data;
      rgb = { r: d[0], g: d[1], b: d[2] };
    } else {
      rgb = null;
    }
  }

  $effect(() => {
    void px.x;
    void px.y;
    void img;
    draw();
  });
</script>

<div class="mag" style:left="{pos.x}px" style:top="{pos.y}px">
  <div class="grid">
    <canvas bind:this={canvas} style:width="{GRID_W}px" style:height="{GRID_H}px"></canvas>
    <span class="band v"></span>
    <span class="band h"></span>
    <span class="center"></span>
  </div>
  <div class="info">
    <span class="num">({mon.x + px.x}, {mon.y + px.y})</span>
    <span class="val"><span class="sw" style:background={swatch}></span><span class="mono">{value}</span></span>
    <span class="hint">{t('P 切换格式 · C 复制颜色')}</span>
  </div>
</div>

<style>
  /* 放大镜看的是截图本身的颜色，两种模式下都用深色信息栏，避免颜色判断受主题影响 */
  .mag {
    position: absolute;
    width: 156px;
    border-radius: var(--r-ctl);
    overflow: hidden;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, .7), 0 8px 24px rgba(0, 0, 0, .35);
    pointer-events: none;
    background: #000;
    z-index: 20;
  }
  .grid {
    position: relative;
    width: 154px;
    height: 98px;
    margin: 1px;
  }
  canvas { display: block; }
  .band {
    position: absolute;
    background: rgba(56, 140, 255, .28);
  }
  .band.v { left: 70px; top: 0; width: 14px; height: 98px; }
  .band.h { left: 0; top: 42px; width: 154px; height: 14px; }
  .center {
    position: absolute;
    left: 70px;
    top: 42px;
    width: 14px;
    height: 14px;
    border: 2px solid #fff;
    box-shadow: 0 0 0 1px #000;
  }
  .info {
    background: rgba(10, 12, 16, .92);
    color: #e8ecf2;
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    line-height: 15px;
  }
  .num { font-variant-numeric: tabular-nums; }
  .val { display: flex; align-items: center; gap: 6px; }
  .sw {
    width: 12px;
    height: 12px;
    border-radius: 3px;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, .4);
  }
  .hint { color: #9ba6b5; }
</style>
