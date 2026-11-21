// 在普通浏览器里打开遮罩页时用的假截图，方便不启动 Tauri 也能调交互。
import type { CaptureStart } from '$lib/ipc';
import type { Point, Rect } from '$lib/geometry';

/** 假截图里「窗口」及其「控件」（标题栏、正文行）的矩形，物理像素。 */
let devRects: Rect[] = [];

/** 假吸附链：包含该点的矩形按面积从小到大（相当于从内到外）。 */
export function devChain(p: Point): Rect[] {
  return devRects
    .filter((r) => p.x >= r.x && p.y >= r.y && p.x < r.x + r.width && p.y < r.y + r.height)
    .sort((a, b) => a.width * a.height - b.width * b.height);
}

export async function devCapture(): Promise<{ start: CaptureStart; url: string }> {
  const scale = devicePixelRatio || 1;
  const width = Math.round(innerWidth * scale);
  const height = Math.round(innerHeight * scale);
  const c = document.createElement('canvas');
  c.width = width;
  c.height = height;
  const g = c.getContext('2d')!;
  const grad = g.createLinearGradient(0, 0, width, height);
  grad.addColorStop(0, '#c7d2e1');
  grad.addColorStop(1, '#8aa4c8');
  g.fillStyle = grad;
  g.fillRect(0, 0, width, height);
  // 几个「窗口」和一些文字，便于看清选区和放大镜
  const wins = [
    [0.08, 0.1, 0.4, 0.45, '#ffffff'],
    [0.52, 0.18, 0.38, 0.5, '#1c222c'],
    [0.2, 0.62, 0.5, 0.3, '#f8fafc'],
  ] as const;
  devRects = [];
  for (const [x, y, w, h, color] of wins) {
    g.fillStyle = color;
    g.fillRect(x * width, y * height, w * width, h * height);
    const win = { x: Math.round(x * width), y: Math.round(y * height), width: Math.round(w * width), height: Math.round(h * height) };
    devRects.push(win, { ...win, height: Math.round(36 * scale) });
    for (let i = 0; i < 6; i++) {
      devRects.push({ x: win.x + Math.round(0.02 * width), y: Math.round((y + 0.06) * height + i * 26 * scale - 18 * scale), width: Math.round(420 * scale), height: Math.round(24 * scale) });
    }
    g.fillStyle = color === '#1c222c' ? '#e8ecf2' : '#0f172a';
    g.font = `${16 * scale}px sans-serif`;
    for (let i = 0; i < 6; i++) {
      g.fillText('The quick brown fox jumps over the lazy dog 敏捷的狐狸', (x + 0.02) * width, (y + 0.06) * height + i * 26 * scale);
    }
  }
  const blob = await new Promise<Blob>((r) => c.toBlob((b) => r(b!), 'image/png'));
  return {
    start: { seq: 1, index: 0, monitor: { x: 0, y: 0, width, height, scale }, hasCursor: true },
    url: URL.createObjectURL(blob),
  };
}
