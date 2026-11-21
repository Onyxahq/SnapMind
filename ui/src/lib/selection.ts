// 选区的纯逻辑：命中测试、拖动调整、移动、微调。
// 单位：所在显示器截图的物理像素（整数），原点为显示器左上角，范围 [0, W] × [0, H]。

import type { Point, Rect } from './geometry';

export type Handle = 'nw' | 'n' | 'ne' | 'e' | 'se' | 's' | 'sw' | 'w';
export type Hit = Handle | 'move' | null;

export const HANDLES: Handle[] = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'];

const clamp = (v: number, lo: number, hi: number) => Math.min(Math.max(v, lo), hi);

/** 由两个角点得到规范化的矩形（宽高非负），并夹在 [0,W]×[0,H] 内。 */
export function fromCorners(a: Point, b: Point, W: number, H: number): Rect {
  const x1 = clamp(Math.min(a.x, b.x), 0, W);
  const y1 = clamp(Math.min(a.y, b.y), 0, H);
  const x2 = clamp(Math.max(a.x, b.x), 0, W);
  const y2 = clamp(Math.max(a.y, b.y), 0, H);
  return { x: x1, y: y1, width: x2 - x1, height: y2 - y1 };
}

/** 控制点在矩形上的位置（四角和四边中点）。 */
export function handlePoint(r: Rect, h: Handle): Point {
  const cx = r.x + r.width / 2;
  const cy = r.y + r.height / 2;
  const R = r.x + r.width;
  const B = r.y + r.height;
  switch (h) {
    case 'nw': return { x: r.x, y: r.y };
    case 'n': return { x: cx, y: r.y };
    case 'ne': return { x: R, y: r.y };
    case 'e': return { x: R, y: cy };
    case 'se': return { x: R, y: B };
    case 's': return { x: cx, y: B };
    case 'sw': return { x: r.x, y: B };
    case 'w': return { x: r.x, y: cy };
  }
}

/**
 * 命中测试。tol 为容差（与矩形同单位）。
 * 优先级：控制点 > 四条边 > 内部（移动）。选区很小时控制点仍可抓到，内部让给移动。
 */
export function hitTest(r: Rect, p: Point, tol: number): Hit {
  for (const h of HANDLES) {
    const q = handlePoint(r, h);
    if (Math.abs(p.x - q.x) <= tol && Math.abs(p.y - q.y) <= tol) {
      // 选区太小时，中点控制点和角点重叠，内部优先给角点
      return h;
    }
  }
  const R = r.x + r.width;
  const B = r.y + r.height;
  const inX = p.x >= r.x - tol && p.x <= R + tol;
  const inY = p.y >= r.y - tol && p.y <= B + tol;
  if (inY && Math.abs(p.x - r.x) <= tol) return 'w';
  if (inY && Math.abs(p.x - R) <= tol) return 'e';
  if (inX && Math.abs(p.y - r.y) <= tol) return 'n';
  if (inX && Math.abs(p.y - B) <= tol) return 's';
  if (p.x > r.x && p.x < R && p.y > r.y && p.y < B) return 'move';
  return null;
}

/**
 * 拖动控制点或边调整大小。从拖动开始时的矩形 start 出发，按位移 (dx, dy) 移动对应的边；
 * 拖过对边时自动翻转（左边拖到右边之外就变成右边），结果夹在显示器内。
 */
export function resize(start: Rect, h: Handle, dx: number, dy: number, W: number, H: number): Rect {
  let l = start.x;
  let t = start.y;
  let r = start.x + start.width;
  let b = start.y + start.height;
  if (h.includes('w')) l += dx;
  if (h.includes('e')) r += dx;
  if (h.includes('n')) t += dy;
  if (h.includes('s')) b += dy;
  return fromCorners({ x: l, y: t }, { x: r, y: b }, W, H);
}

/** 整体移动，保持尺寸，碰到显示器边缘就停住。 */
export function move(start: Rect, dx: number, dy: number, W: number, H: number): Rect {
  return {
    ...start,
    x: clamp(start.x + dx, 0, W - start.width),
    y: clamp(start.y + dy, 0, H - start.height),
  };
}

/** 方向键微调：普通 1px，按住 Shift 10px。返回 null 表示不是方向键。 */
export function nudgeDelta(key: string, shift: boolean): Point | null {
  const s = shift ? 10 : 1;
  switch (key) {
    case 'ArrowLeft': return { x: -s, y: 0 };
    case 'ArrowRight': return { x: s, y: 0 };
    case 'ArrowUp': return { x: 0, y: -s };
    case 'ArrowDown': return { x: 0, y: s };
    default: return null;
  }
}

/** 命中结果对应的鼠标指针。 */
export function cursorFor(hit: Hit): string {
  switch (hit) {
    case 'nw':
    case 'se': return 'nwse-resize';
    case 'ne':
    case 'sw': return 'nesw-resize';
    case 'n':
    case 's': return 'ns-resize';
    case 'e':
    case 'w': return 'ew-resize';
    case 'move': return 'move';
    default: return 'crosshair';
  }
}

/** 尺寸标签的位置（CSS 像素）：默认在选区上方 28px，贴近屏幕顶端时放进选区内。 */
export function labelTop(selTopCss: number): number {
  return selTopCss - 28 < 4 ? selTopCss + 6 : selTopCss - 28;
}
