// 原位覆盖（需求第 4 节、M6.2）：译文按段落画在原文位置上。
// 屏幕上的覆盖层和复制 / 贴图时导出的图片用同一套排版（截图物理像素），两边看起来一样。
import { wrapText } from './pin';

/** Rust 发来的段落块（坐标相对选区，截图物理像素）。 */
export interface OverlayBlock {
  x: number;
  y: number;
  width: number;
  height: number;
  fg: [number, number, number];
  bg: [number, number, number];
  /** 原文字号估计（笔画高度，像素） */
  size: number;
  /** 公式、代码、纯数字符号：保持原样不覆盖 */
  keep: boolean;
  src: string;
  dst: string;
}

export const OVERLAY_FONT = `'Microsoft YaHei UI', 'Segoe UI', sans-serif`;
export const LINE_HEIGHT = 1.25;
/** 最小字号（物理像素），再小就看不清了；放不下时允许超出块的下边 */
export const MIN_SIZE = 9;

export interface Fitted {
  size: number;
  lines: string[];
  /** 文字左上角相对块左上角的偏移 */
  dx: number;
  dy: number;
}

/**
 * 在块里排下译文：从原文字号起步，放不下每次缩小 8%，到最小字号为止。
 * `measure(text, size)` 返回这段文字在该字号下的宽度。
 */
export function fitText(b: OverlayBlock, measure: (s: string, size: number) => number): Fitted {
  const pad = Math.ceil(b.size * 0.25);
  const w = Math.max(1, b.width - pad * 2);
  // 块四周各留了 0.25 个字号；文字可以吃掉一半留白（上下合起来一个 pad）
  const avail = Math.max(1, b.height - pad);
  let size = Math.max(MIN_SIZE, Math.min(b.size / 0.95, avail));
  for (;;) {
    const lines = wrapText(b.dst.trim(), w, (s) => measure(s, size));
    // 文字实际占的高度：n 行的行高减去最后一行多出的行距
    const extent = size * (lines.length * LINE_HEIGHT - (LINE_HEIGHT - 1));
    if (extent <= avail || size <= MIN_SIZE) {
      // 纵向居中（单行时和原文的位置基本一致）
      const total = lines.length * size * LINE_HEIGHT;
      return { size, lines, dx: pad, dy: (b.height - total) / 2 };
    }
    size = Math.max(MIN_SIZE, size * 0.92);
  }
}

export const rgb = (c: [number, number, number]) => `rgb(${c[0]}, ${c[1]}, ${c[2]})`;

/** 要画出来的块：不保持原样、译文不为空、译文和原文不同。 */
export function drawable(blocks: OverlayBlock[]): OverlayBlock[] {
  return blocks.filter((b) => !b.keep && b.dst.trim() && b.dst.trim() !== b.src.trim());
}

/** 画到画布上（坐标系已经平移到选区左上角）。 */
export function drawOverlay(g: CanvasRenderingContext2D, blocks: OverlayBlock[], fits: Fitted[]) {
  g.save();
  g.textBaseline = 'middle';
  blocks.forEach((b, i) => {
    const f = fits[i];
    g.fillStyle = rgb(b.bg);
    g.fillRect(b.x, b.y, b.width, b.height);
    g.fillStyle = rgb(b.fg);
    g.font = `${f.size}px ${OVERLAY_FONT}`;
    f.lines.forEach((l, k) => g.fillText(l, b.x + f.dx, b.y + f.dy + (k + 0.5) * f.size * LINE_HEIGHT));
  });
  g.restore();
}

/** 用画布量文字宽度（字号会变，按字号缓存 font 设置）。 */
export function canvasMeasure(): (s: string, size: number) => number {
  const g = new OffscreenCanvas(1, 1).getContext('2d')!;
  let last = -1;
  return (s, size) => {
    if (size !== last) {
      g.font = `${size}px ${OVERLAY_FONT}`;
      last = size;
    }
    return g.measureText(s).width;
  };
}
