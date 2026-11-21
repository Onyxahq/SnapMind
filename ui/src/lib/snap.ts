// 吸附链的前端处理：Rust 给的是虚拟桌面物理坐标，这里裁到当前显示器并换成本屏像素。

import type { MonitorGeom, Point, Rect } from './geometry';

const same = (a: Rect, b: Rect) =>
  a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;

/** 裁到显示器内、换成本屏像素坐标；裁完为空或与前一个相同的去掉。顺序保持「从内到外」。 */
export function toLocalChain(chain: Rect[], m: MonitorGeom): Rect[] {
  const out: Rect[] = [];
  for (const r of chain) {
    const x1 = Math.max(r.x - m.x, 0);
    const y1 = Math.max(r.y - m.y, 0);
    const x2 = Math.min(r.x + r.width - m.x, m.width);
    const y2 = Math.min(r.y + r.height - m.y, m.height);
    if (x2 <= x1 || y2 <= y1) continue;
    const c = { x: x1, y: y1, width: x2 - x1, height: y2 - y1 };
    if (!out.length || !same(out[out.length - 1], c)) out.push(c);
  }
  return out;
}

export function containsPoint(r: Rect, p: Point): boolean {
  return p.x >= r.x && p.y >= r.y && p.x < r.x + r.width && p.y < r.y + r.height;
}

/**
 * 鼠标移动后拿到新的吸附链时，决定高亮哪一层：
 * 如果当前高亮的矩形仍在新链里（例如已经滚到「整个窗口」，鼠标还在窗口里），保持它；
 * 否则回到最内层。
 */
export function keepLevel(newChain: Rect[], current: Rect | null): number {
  if (!current) return 0;
  const i = newChain.findIndex((r) => same(r, current));
  return i >= 0 ? i : 0;
}

/** 滚轮：向上（deltaY < 0）往外一层，向下往里一层，夹在链的范围内。 */
export function wheelLevel(level: number, deltaY: number, length: number): number {
  if (length === 0) return 0;
  const next = deltaY < 0 ? level + 1 : deltaY > 0 ? level - 1 : level;
  return Math.min(Math.max(next, 0), length - 1);
}
