// 标注：矢量数据模型、命中测试、撤销历史与 Canvas 绘制。
// 坐标和粗细都用所在显示器截图的物理像素，导出时（M2.3）可以按原始分辨率重新绘制。

import type { Point } from './geometry';

/** 工具，顺序即快捷键 1–7。 */
export type Tool = 'rect' | 'pen' | 'arrow' | 'text' | 'number' | 'mosaic' | 'eraser';
export const TOOLS: Tool[] = ['rect', 'pen', 'arrow', 'text', 'number', 'mosaic', 'eraser'];
/** 当前已实现、出现在工具条上的工具。 */
export const READY_TOOLS: Tool[] = ['rect', 'pen', 'arrow', 'text', 'number', 'mosaic', 'eraser'];

/** props：有没有样式弹层；colors：弹层里有没有颜色（马赛克 / 模糊没有颜色）。 */
export const TOOL_INFO: Record<Tool, { label: string; subs: string[] | null; props: boolean; colors: boolean }> = {
  rect: { label: '矩形', subs: ['矩形', '椭圆'], props: true, colors: true },
  pen: { label: '画笔', subs: ['画笔', '荧光笔'], props: true, colors: true },
  arrow: { label: '箭头', subs: ['箭头', '直线'], props: true, colors: true },
  text: { label: '文字', subs: null, props: true, colors: true },
  number: { label: '序号', subs: null, props: true, colors: true },
  mosaic: { label: '马赛克', subs: ['马赛克', '模糊'], props: true, colors: false },
  eraser: { label: '橡皮擦', subs: null, props: false, colors: false },
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
/** 文字的三档字号（CSS 像素）。 */
export const FONT_SIZES = [16, 22, 30];
/** 马赛克方块边长 / 模糊半径的三档（CSS 像素）。 */
export const MOSAIC_SIZES = [6, 10, 16];
export const TEXT_FONT = "500 {px}px Inter, 'Segoe UI', 'Microsoft YaHei UI', sans-serif";
export const TEXT_LINE_HEIGHT = 1.3;
export const textFont = (px: number) => TEXT_FONT.replace('{px}', String(px));

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
  | (Base & { kind: 'number'; n: number; at: Point })
  /** 文字：at 为左上角；width 字段存字号；box 是提交时量出的尺寸（命中测试用） */
  | (Base & { kind: 'text'; text: string; at: Point; box: { w: number; h: number } })
  /** 马赛克 / 模糊：width 字段存方块边长或模糊半径；基于截图真实像素绘制 */
  | (Base & { kind: 'mosaic'; blur: boolean; a: Point; b: Point });

export type TextShape = Extract<Shape, { kind: 'text' }>;

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
    case 'text':
      return (
        p.x >= s.at.x - tol && p.x <= s.at.x + s.box.w + tol && p.y >= s.at.y - tol && p.y <= s.at.y + s.box.h + tol
      );
    case 'mosaic': {
      // 马赛克是实心的，整块区域都算
      const x1 = Math.min(s.a.x, s.b.x), x2 = Math.max(s.a.x, s.b.x);
      const y1 = Math.min(s.a.y, s.b.y), y2 = Math.max(s.a.y, s.b.y);
      return p.x >= x1 - tol && p.x <= x2 + tol && p.y >= y1 - tol && p.y <= y2 + tol;
    }
  }
}

/** 在若干形状里找最上面（最后画的）一个被点中的文字。 */
export function textAt(shapes: Shape[], p: Point, tol: number): TextShape | null {
  for (let i = shapes.length - 1; i >= 0; i--) {
    const s = shapes[i];
    if (s.kind === 'text' && hitShape(s, p, tol)) return s;
  }
  return null;
}

// ---------------------------------------------------------------- 历史

/** 一步操作：添加 / 删除若干形状，或把一个形状换成修改后的版本（改文字、拖动文字）。
 * 撤销就是反向执行最后一步。 */
export type Op =
  | { type: 'add'; shapes: Shape[] }
  | { type: 'remove'; shapes: Shape[] }
  | { type: 'replace'; before: Shape; after: Shape };

export interface Doc {
  shapes: Shape[];
  history: Op[];
}

export const emptyDoc = (): Doc => ({ shapes: [], history: [] });

export function apply(doc: Doc, op: Op): Doc {
  if (op.type === 'replace') {
    if (op.before === op.after) return doc;
    return { shapes: doc.shapes.map((s) => (s.id === op.before.id ? op.after : s)), history: [...doc.history, op] };
  }
  if (op.shapes.length === 0) return doc;
  const ids = new Set(op.shapes.map((s) => s.id));
  const shapes = op.type === 'add' ? [...doc.shapes, ...op.shapes] : doc.shapes.filter((s) => !ids.has(s.id));
  return { shapes, history: [...doc.history, op] };
}

export function undo(doc: Doc): Doc {
  const op = doc.history[doc.history.length - 1];
  if (!op) return doc;
  if (op.type === 'replace') {
    return { shapes: doc.shapes.map((s) => (s.id === op.after.id ? op.before : s)), history: doc.history.slice(0, -1) };
  }
  const ids = new Set(op.shapes.map((s) => s.id));
  // 删除的形状按原来的先后顺序放回去（按 id 排序即创建顺序）
  const shapes =
    op.type === 'add'
      ? doc.shapes.filter((s) => !ids.has(s.id))
      : [...doc.shapes, ...op.shapes].sort((a, b) => a.id - b.id);
  return { shapes, history: doc.history.slice(0, -1) };
}

// ---------------------------------------------------------------- 绘制

/** 马赛克用的小画布（复用，避免每帧新建）。 */
let scratch: HTMLCanvasElement | OffscreenCanvas | null = null;
function scratchCanvas(w: number, h: number) {
  if (!scratch) scratch = typeof OffscreenCanvas !== 'undefined' ? new OffscreenCanvas(w, h) : document.createElement('canvas');
  if (scratch.width < w) scratch.width = w;
  if (scratch.height < h) scratch.height = h;
  return scratch;
}

/** 马赛克 / 模糊：从截图（src，像素坐标与形状一致）取真实像素处理后画到区域里。 */
function drawMosaic(g: CanvasRenderingContext2D, s: Extract<Shape, { kind: 'mosaic' }>, src: CanvasImageSource, k: number) {
  const x = Math.round(Math.min(s.a.x, s.b.x)), y = Math.round(Math.min(s.a.y, s.b.y));
  const w = Math.round(Math.abs(s.b.x - s.a.x)), h = Math.round(Math.abs(s.b.y - s.a.y));
  if (w < 1 || h < 1) return;
  g.save();
  g.beginPath();
  g.rect(x * k, y * k, w * k, h * k);
  g.clip();
  if (s.blur) {
    // 模糊：多取一圈像素再模糊，避免边缘发虚透明
    const r = s.width;
    g.filter = `blur(${r * k}px)`;
    g.drawImage(src, x - r, y - r, w + 2 * r, h + 2 * r, (x - r) * k, (y - r) * k, (w + 2 * r) * k, (h + 2 * r) * k);
  } else {
    // 马赛克：先缩小（每个方块取平均色），再不平滑地放大
    const block = Math.max(2, Math.round(s.width));
    const sw = Math.ceil(w / block), sh = Math.ceil(h / block);
    const c = scratchCanvas(sw, sh);
    const cg = c.getContext('2d') as CanvasRenderingContext2D;
    cg.imageSmoothingEnabled = true;
    cg.imageSmoothingQuality = 'medium';
    cg.clearRect(0, 0, sw, sh);
    cg.drawImage(src, x, y, sw * block, sh * block, 0, 0, sw, sh);
    g.imageSmoothingEnabled = false;
    g.drawImage(c as CanvasImageSource, 0, 0, sw, sh, x * k, y * k, sw * block * k, sh * block * k);
  }
  g.restore();
}

/** 把形状画到画布上。k = 画布像素 / 截图物理像素；src 是冻结帧（马赛克 / 模糊要用真实像素）。 */
export function drawShape(g: CanvasRenderingContext2D, s: Shape, k: number, src?: CanvasImageSource): void {
  if (s.kind === 'mosaic') {
    if (src) drawMosaic(g, s, src, k);
    return;
  }
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
    case 'text': {
      g.font = textFont(s.width);
      g.textBaseline = 'top';
      const lh = s.width * TEXT_LINE_HEIGHT;
      // 行高多出来的部分上下各分一半，和编辑框里的排版一致
      const pad = (lh - s.width) / 2;
      s.text.split('\n').forEach((line, i) => g.fillText(line, s.at.x, s.at.y + pad + i * lh));
      break;
    }
  }
  g.restore();
}

/** 量文字的框（物理像素），用于命中测试。 */
export function measureText(text: string, fontPx: number): { w: number; h: number } {
  const c = scratchCanvas(1, 1);
  const g = c.getContext('2d') as CanvasRenderingContext2D;
  g.font = textFont(fontPx);
  const lines = text.split('\n');
  const w = Math.max(...lines.map((l) => g.measureText(l).width), fontPx * 0.5);
  return { w: Math.ceil(w), h: Math.ceil(lines.length * fontPx * TEXT_LINE_HEIGHT) };
}

/** 标注层：相对选区左上角的偏移 + RGBA 像素（非预乘 alpha）。 */
export interface Layer {
  x: number;
  y: number;
  width: number;
  height: number;
  rgba: Uint8Array;
}

/** 找出 RGBA 里不透明像素的外框；全透明返回 null。 */
export function alphaBounds(rgba: Uint8ClampedArray | Uint8Array, width: number, height: number) {
  let x1 = width, y1 = height, x2 = -1, y2 = -1;
  for (let y = 0; y < height; y++) {
    const row = y * width * 4;
    for (let x = 0; x < width; x++) {
      if (rgba[row + x * 4 + 3] !== 0) {
        if (x < x1) x1 = x;
        if (x > x2) x2 = x;
        if (y < y1) y1 = y;
        y2 = y;
      }
    }
  }
  return x2 < 0 ? null : { x: x1, y: y1, width: x2 - x1 + 1, height: y2 - y1 + 1 };
}

/**
 * 按原始分辨率渲染标注层（导出用）：1 个画布像素 = 1 个截图物理像素，与屏幕上用的是同一套
 * 绘制代码，所见即所得。只返回有内容的那一块（裁到不透明像素的外框），上传量与标注面积成正比；
 * 不编码 PNG，Rust 端也不用解码。没有可见内容时返回 null。
 */
export function renderLayer(
  shapes: Shape[],
  sel: { x: number; y: number; width: number; height: number },
  src: CanvasImageSource,
): Layer | null {
  const width = Math.max(1, sel.width);
  const height = Math.max(1, sel.height);
  const c = new OffscreenCanvas(width, height);
  const g = c.getContext('2d', { willReadFrequently: true }) as unknown as CanvasRenderingContext2D;
  g.translate(-sel.x, -sel.y);
  for (const s of shapes) drawShape(g, s, 1, src);
  const full = g.getImageData(0, 0, width, height).data;
  const b = alphaBounds(full, width, height);
  if (!b) return null;
  const part = g.getImageData(b.x, b.y, b.width, b.height).data;
  return { ...b, rgba: new Uint8Array(part.buffer, part.byteOffset, part.byteLength) };
}
