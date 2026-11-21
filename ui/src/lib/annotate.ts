// 标注：矢量数据模型、命中测试、撤销历史与 Canvas 绘制。
// 坐标和粗细都用所在显示器截图的物理像素，导出时（M2.3）可以按原始分辨率重新绘制。

import type { Point } from './geometry';

/** 工具，顺序即快捷键 1–7。text、mosaic 在 M2.2 实现。 */
export type Tool = 'rect' | 'pen' | 'arrow' | 'text' | 'number' | 'mosaic' | 'eraser';
export const TOOLS: Tool[] = ['rect', 'pen', 'arrow', 'text', 'number', 'mosaic', 'eraser'];
/** 当前已实现、出现在工具条上的工具。 */
export const READY_TOOLS: Tool[] = ['rect', 'pen', 'arrow', 'number', 'eraser'];

export const TOOL_INFO: Record<Tool, { label: string; subs: string[] | null; props: boolean }> = {
  rect: { label: '矩形', subs: ['矩形', '椭圆'], props: true },
  pen: { label: '画笔', subs: ['画笔', '荧光笔'], props: true },
  arrow: { label: '箭头', subs: ['箭头', '直线'], props: true },
  text: { label: '文字', subs: null, props: true },
  number: { label: '序号', subs: null, props: true },
  mosaic: { label: '马赛克', subs: ['马赛克', '模糊'], props: true },
  eraser: { label: '橡皮擦', subs: null, props: false },
};

export const COLORS: { value: string; label: string }[] = [
  { value: '#EF4444', label: '红' },
  { value: '#F59E0B', label: '黄' },
  { value: '#22C55E', label: '绿' },
  { value: '#2563EB', label: '蓝' },
  { value: '#FFFFFF', label: '白' },
  { value: '#0F172A', label: '黑' },
];
/** 三档粗细（CSS 像素，存储时乘以显示器缩放）。 */
export const SIZES = [2, 4, 6];

/** 每个工具记住自己的子类型、颜色和粗细。 */
export interface ToolOpts {
  sub: number;
  color: number;
  size: number;
}
export const defaultOpts = (): Record<Tool, ToolOpts> =>
  Object.fromEntries(TOOLS.map((t) => [t, { sub: 0, color: 0, size: 1 }])) as Record<Tool, ToolOpts>;

/** 1–7 数字键 → 工具（未实现的返回 null）。 */
export function toolForKey(key: string): Tool | null {
  const i = Number(key) - 1;
  if (!Number.isInteger(i) || i < 0 || i >= TOOLS.length) return null;
  const t = TOOLS[i];
  return READY_TOOLS.includes(t) ? t : null;
}

// ---------------------------------------------------------------- 形状

interface Base {
  id: number;
  color: string;
  /** 线宽，物理像素 */
  width: number;
}
export type Shape =
  | (Base & { kind: 'rect'; ellipse: boolean; a: Point; b: Point })
  | (Base & { kind: 'pen'; highlighter: boolean; points: Point[] })
  | (Base & { kind: 'arrow'; line: boolean; a: Point; b: Point })
  | (Base & { kind: 'number'; n: number; at: Point });

/** 按住 Shift：矩形变正方形 / 圆，直线吸附到 45° 的倍数。 */
export function constrain(kind: 'rect' | 'arrow', a: Point, b: Point): Point {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  if (kind === 'rect') {
    const s = Math.max(Math.abs(dx), Math.abs(dy));
    return { x: a.x + Math.sign(dx || 1) * s, y: a.y + Math.sign(dy || 1) * s };
  }
  const len = Math.hypot(dx, dy);
  const ang = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4);
  return { x: a.x + Math.round(Math.cos(ang) * len), y: a.y + Math.round(Math.sin(ang) * len) };
}

/** 箭头头部的三个点（尖端、两翼），大小随线宽变化。 */
export function arrowHead(a: Point, b: Point, width: number): [Point, Point, Point] {
  const len = Math.hypot(b.x - a.x, b.y - a.y) || 1;
  const head = Math.min(Math.max(width * 4 + 6, 10), len * 0.6);
  const ang = Math.atan2(b.y - a.y, b.x - a.x);
  const spread = Math.PI / 7;
  return [
    b,
    { x: b.x - head * Math.cos(ang - spread), y: b.y - head * Math.sin(ang - spread) },
    { x: b.x - head * Math.cos(ang + spread), y: b.y - head * Math.sin(ang + spread) },
  ];
}

/** 序号圆的半径（物理像素）。 */
export const numberRadius = (width: number) => width * 2.5 + 8;

/** 下一个序号：现有最大值 + 1（撤销或擦掉后自然回退）。 */
export function nextNumber(shapes: Shape[]): number {
  return shapes.reduce((m, s) => (s.kind === 'number' ? Math.max(m, s.n) : m), 0) + 1;
}

/** 画笔点太密时丢掉与上一个点距离不足 min 的点，减小数据量又不影响观感。 */
export function addPoint(points: Point[], p: Point, min = 1.5): Point[] {
  const last = points[points.length - 1];
  if (last && Math.hypot(p.x - last.x, p.y - last.y) < min) return points;
  return [...points, p];
}

function distToSegment(p: Point, a: Point, b: Point): number {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const l2 = dx * dx + dy * dy;
  const t = l2 ? Math.max(0, Math.min(1, ((p.x - a.x) * dx + (p.y - a.y) * dy) / l2)) : 0;
  return Math.hypot(p.x - (a.x + t * dx), p.y - (a.y + t * dy));
}

/** 橡皮擦命中：点到形状描边的距离在 tol 以内。矩形 / 椭圆只算边框，不算内部。 */
export function hitShape(s: Shape, p: Point, tol: number): boolean {
  const t = tol + s.width / 2;
  switch (s.kind) {
    case 'rect': {
      const x1 = Math.min(s.a.x, s.b.x), x2 = Math.max(s.a.x, s.b.x);
      const y1 = Math.min(s.a.y, s.b.y), y2 = Math.max(s.a.y, s.b.y);
      if (s.ellipse) {
        const rx = (x2 - x1) / 2 || 1, ry = (y2 - y1) / 2 || 1;
        const cx = x1 + rx, cy = y1 + ry;
        const d = Math.hypot((p.x - cx) / rx, (p.y - cy) / ry);
        return Math.abs(d - 1) * Math.min(rx, ry) <= t;
      }
      const corners = [{ x: x1, y: y1 }, { x: x2, y: y1 }, { x: x2, y: y2 }, { x: x1, y: y2 }];
      return corners.some((c, i) => distToSegment(p, c, corners[(i + 1) % 4]) <= t);
    }
    case 'pen': {
      const w = s.highlighter ? t + s.width * 1.5 : t;
      if (s.points.length === 1) return Math.hypot(p.x - s.points[0].x, p.y - s.points[0].y) <= w;
      return s.points.some((q, i) => i > 0 && distToSegment(p, s.points[i - 1], q) <= w);
    }
    case 'arrow':
      return distToSegment(p, s.a, s.b) <= t;
    case 'number':
      return Math.hypot(p.x - s.at.x, p.y - s.at.y) <= numberRadius(s.width) + tol;
  }
}

// ---------------------------------------------------------------- 历史

/** 一步操作：添加若干形状或删除若干形状。撤销就是反向执行最后一步。 */
export type Op = { type: 'add'; shapes: Shape[] } | { type: 'remove'; shapes: Shape[] };

export interface Doc {
  shapes: Shape[];
  history: Op[];
}

export const emptyDoc = (): Doc => ({ shapes: [], history: [] });

export function apply(doc: Doc, op: Op): Doc {
  if (op.shapes.length === 0) return doc;
  const ids = new Set(op.shapes.map((s) => s.id));
  const shapes = op.type === 'add' ? [...doc.shapes, ...op.shapes] : doc.shapes.filter((s) => !ids.has(s.id));
  return { shapes, history: [...doc.history, op] };
}

export function undo(doc: Doc): Doc {
  const op = doc.history[doc.history.length - 1];
  if (!op) return doc;
  const ids = new Set(op.shapes.map((s) => s.id));
  // 删除的形状按原来的先后顺序放回去（按 id 排序即创建顺序）
  const shapes =
    op.type === 'add'
      ? doc.shapes.filter((s) => !ids.has(s.id))
      : [...doc.shapes, ...op.shapes].sort((a, b) => a.id - b.id);
  return { shapes, history: doc.history.slice(0, -1) };
}

// ---------------------------------------------------------------- 绘制

/** 把形状画到画布上。k = 画布像素 / 截图物理像素。 */
export function drawShape(g: CanvasRenderingContext2D, s: Shape, k: number): void {
  g.save();
  g.scale(k, k);
  g.strokeStyle = s.color;
  g.fillStyle = s.color;
  g.lineWidth = s.width;
  g.lineCap = 'round';
  g.lineJoin = 'round';
  switch (s.kind) {
    case 'rect': {
      const x = Math.min(s.a.x, s.b.x), y = Math.min(s.a.y, s.b.y);
      const w = Math.abs(s.b.x - s.a.x), h = Math.abs(s.b.y - s.a.y);
      g.beginPath();
      if (s.ellipse) g.ellipse(x + w / 2, y + h / 2, w / 2, h / 2, 0, 0, Math.PI * 2);
      else g.rect(x, y, w, h);
      g.stroke();
      break;
    }
    case 'pen': {
      if (s.highlighter) {
        g.globalAlpha = 0.38;
        g.lineWidth = s.width * 4;
        g.lineCap = 'square';
      }
      const pts = s.points;
      g.beginPath();
      g.moveTo(pts[0].x, pts[0].y);
      if (pts.length === 1) g.lineTo(pts[0].x + 0.01, pts[0].y);
      // 经过相邻两点中点的二次曲线，笔迹更圆润
      for (let i = 1; i < pts.length - 1; i++) {
        const mx = (pts[i].x + pts[i + 1].x) / 2, my = (pts[i].y + pts[i + 1].y) / 2;
        g.quadraticCurveTo(pts[i].x, pts[i].y, mx, my);
      }
      if (pts.length > 1) g.lineTo(pts[pts.length - 1].x, pts[pts.length - 1].y);
      g.stroke();
      break;
    }
    case 'arrow': {
      g.beginPath();
      if (s.line) {
        g.moveTo(s.a.x, s.a.y);
        g.lineTo(s.b.x, s.b.y);
        g.stroke();
        break;
      }
      const [tip, l, r] = arrowHead(s.a, s.b, s.width);
      // 杆子只画到箭头根部，避免粗线从尖端冒出来
      const base = { x: (l.x + r.x) / 2, y: (l.y + r.y) / 2 };
      g.moveTo(s.a.x, s.a.y);
      g.lineTo(base.x, base.y);
      g.stroke();
      g.beginPath();
      g.moveTo(tip.x, tip.y);
      g.lineTo(l.x, l.y);
      g.lineTo(r.x, r.y);
      g.closePath();
      g.fill();
      break;
    }
    case 'number': {
      const r = numberRadius(s.width);
      g.beginPath();
      g.arc(s.at.x, s.at.y, r, 0, Math.PI * 2);
      g.fill();
      g.fillStyle = s.color.toUpperCase() === '#FFFFFF' ? '#0F172A' : '#FFFFFF';
      g.font = `600 ${Math.round(r * 1.15)}px Inter, 'Segoe UI', sans-serif`;
      g.textAlign = 'center';
      g.textBaseline = 'middle';
      g.fillText(String(s.n), s.at.x, s.at.y + r * 0.06);
      break;
    }
  }
  g.restore();
}
