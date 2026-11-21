// 标注控制器：截图编辑态和贴图共用的一套标注状态与交互（工具、样式、撤销、画、擦、文字编辑与拖动）。
// 坐标一律是「画布的物理像素」（截图编辑态 = 本屏截图像素，贴图 = 贴图图片像素），
// 宿主负责把鼠标坐标换算进来，并提供把点夹进可绘制区域的函数。

import type { Point } from './geometry';
import {
  COLORS,
  FONT_SIZES,
  MOSAIC_SIZES,
  SIZES,
  TEXT_LINE_HEIGHT,
  addPoint,
  apply,
  constrain,
  defaultOpts,
  emptyDoc,
  hitShape,
  measureText,
  nextNumber,
  textAt,
  undo as undoDoc,
  type Doc,
  type Shape,
  type TextShape,
  type Tool,
  type ToolOpts,
} from './annotate';

/** 正在进行的手势：画一笔、橡皮擦划过、拖动文字。 */
export type Gesture =
  | { kind: 'draw' }
  | { kind: 'erase'; removed: Shape[] }
  | { kind: 'moveText'; orig: TextShape; cur: TextShape; p0: Point; moved: boolean };

export interface AnnotatorEnv {
  /** 1 个 CSS 像素对应多少画布像素（线宽、字号、命中容差按它换算） */
  unit: () => number;
  /** 把点夹进可绘制区域 */
  clamp: (p: Point) => Point;
}

/** 移动超过这么多 CSS 像素才算拖动文字，否则算单击编辑。 */
const MOVE_SLOP = 3;

export class Annotator {
  /** 当前工具；null 表示不在标注模式 */
  tool = $state<Tool | null>(null);
  lastTool: Tool = 'rect';
  /** 每个工具各自记住子类型、颜色、粗细 */
  opts = $state(defaultOpts());
  doc = $state.raw<Doc>(emptyDoc());
  /** 正在画的那一笔 */
  draft = $state.raw<Shape | null>(null);
  /** 正在编辑的文字；original 是被重新编辑的已有文字（新建时为 null） */
  editing = $state.raw<{ shape: TextShape; original: TextShape | null } | null>(null);
  gesture = $state.raw<Gesture | null>(null);

  /** 画布上显示的形状：橡皮擦划过的先藏起来（松手后一次删除），拖动中的文字显示新位置，
   *  正在编辑的文字由编辑框显示。 */
  visible = $derived.by(() => {
    const g = this.gesture;
    let list = this.doc.shapes;
    if (g?.kind === 'erase') list = list.filter((s) => !g.removed.includes(s));
    if (g?.kind === 'moveText') list = list.map((s) => (s.id === g.orig.id ? g.cur : s));
    const ed = this.editing;
    if (ed?.original) list = list.filter((s) => s.id !== ed.original!.id);
    return list;
  });

  private seq = 0;
  private start: Point = { x: 0, y: 0 };

  constructor(private env: AnnotatorEnv) {}

  get active(): boolean {
    return this.tool !== null;
  }
  get canUndo(): boolean {
    return this.doc.history.length > 0;
  }
  get hasShapes(): boolean {
    return this.doc.shapes.length > 0;
  }

  /** 清空（新的一次截图）；工具样式保留。 */
  reset(): void {
    this.tool = null;
    this.doc = emptyDoc();
    this.draft = null;
    this.editing = null;
    this.gesture = null;
  }

  setTool(t: Tool): void {
    if (t !== 'text') this.commitText();
    this.tool = t;
    this.lastTool = t;
  }

  exit(): void {
    this.commitText();
    this.tool = null;
  }

  setOpts(t: Tool, o: Partial<ToolOpts>): void {
    this.opts = { ...this.opts, [t]: { ...this.opts[t], ...o } };
    // 编辑文字时改颜色 / 字号，立即作用到正在编辑的文字
    if (t === 'text' && this.editing) {
      this.editing = {
        ...this.editing,
        shape: { ...this.editing.shape, color: COLORS[this.opts.text.color].value, width: FONT_SIZES[this.opts.text.size] * this.env.unit() },
      };
    }
  }

  undo(): void {
    if (this.gesture) return;
    this.doc = undoDoc(this.doc);
  }

  private style(t: Tool) {
    return { color: COLORS[this.opts[t].color].value, width: SIZES[this.opts[t].size] * this.env.unit() };
  }

  /** 按下：开始一个手势。返回 true 表示开始了拖动类手势，宿主应捕获指针。 */
  down(raw: Point, shift: boolean): boolean {
    const tool = this.tool;
    if (!tool) return false;
    if (this.editing) {
      // 点别处：提交正在编辑的文字（编辑框失焦也会提交，提交是幂等的）
      this.commitText();
      return false;
    }
    const p = this.env.clamp(raw);
    const u = this.env.unit();
    this.start = p;
    const id = ++this.seq;
    switch (tool) {
      case 'rect':
        this.draft = { id, kind: 'rect', ellipse: this.opts.rect.sub === 1, a: p, b: p, ...this.style('rect') };
        break;
      case 'arrow':
        this.draft = { id, kind: 'arrow', line: this.opts.arrow.sub === 1, a: p, b: p, ...this.style('arrow') };
        break;
      case 'pen':
        this.draft = { id, kind: 'pen', highlighter: this.opts.pen.sub === 1, points: [p], ...this.style('pen') };
        break;
      case 'mosaic':
        this.draft = {
          id,
          kind: 'mosaic',
          blur: this.opts.mosaic.sub === 1,
          a: p,
          b: p,
          color: '',
          width: MOSAIC_SIZES[this.opts.mosaic.size] * u,
        };
        break;
      case 'text': {
        const hit = textAt(this.doc.shapes, p, 4 * u);
        if (hit) {
          // 按在已有文字上：拖动则移动，单击则编辑
          this.gesture = { kind: 'moveText', orig: hit, cur: hit, p0: p, moved: false };
          return true;
        }
        const width = FONT_SIZES[this.opts.text.size] * u;
        // 让第一行文字的竖直中心落在点击处
        const at = { x: p.x, y: p.y - (width * TEXT_LINE_HEIGHT) / 2 };
        this.editing = {
          shape: { id, kind: 'text', text: '', at, color: COLORS[this.opts.text.color].value, width, box: { w: 0, h: 0 } },
          original: null,
        };
        return false;
      }
      case 'number': {
        // 序号单击即放下，不需要拖动
        const shape: Shape = { id, kind: 'number', n: nextNumber(this.doc.shapes), at: p, ...this.style('number') };
        this.doc = apply(this.doc, { type: 'add', shapes: [shape] });
        return false;
      }
      case 'eraser':
        this.gesture = { kind: 'erase', removed: [] };
        this.move(p, shift);
        return true;
    }
    this.gesture = { kind: 'draw' };
    return true;
  }

  move(raw: Point, shift: boolean): void {
    const g = this.gesture;
    if (!g) return;
    const p = this.env.clamp(raw);
    const u = this.env.unit();
    if (g.kind === 'erase') {
      const hit = this.doc.shapes.filter((s) => !g.removed.includes(s) && hitShape(s, p, 6 * u));
      if (hit.length) this.gesture = { kind: 'erase', removed: [...g.removed, ...hit] };
      return;
    }
    if (g.kind === 'moveText') {
      const dx = p.x - g.p0.x, dy = p.y - g.p0.y;
      if (!g.moved && Math.hypot(dx, dy) < MOVE_SLOP * u) return;
      this.gesture = { ...g, moved: true, cur: { ...g.orig, at: { x: g.orig.at.x + dx, y: g.orig.at.y + dy } } };
      return;
    }
    const d = this.draft;
    if (!d) return;
    switch (d.kind) {
      case 'rect':
      case 'arrow':
        this.draft = { ...d, b: shift ? constrain(d.kind, this.start, p) : p };
        break;
      case 'mosaic':
        this.draft = { ...d, b: shift ? constrain('rect', this.start, p) : p };
        break;
      case 'pen':
        this.draft = { ...d, points: addPoint(d.points, p, u) };
        break;
    }
  }

  up(): void {
    const g = this.gesture;
    if (g?.kind === 'erase') {
      this.doc = apply(this.doc, { type: 'remove', shapes: g.removed });
    } else if (g?.kind === 'moveText') {
      // 拖动了就是移动；没动就是点一下，进入编辑
      if (g.moved) this.doc = apply(this.doc, { type: 'replace', before: g.orig, after: g.cur });
      else this.editing = { shape: g.orig, original: g.orig };
    } else if (this.draft) {
      const d = this.draft;
      // 太小的形状（误触）不保留
      const tiny =
        (d.kind === 'rect' || d.kind === 'arrow' || d.kind === 'mosaic') &&
        Math.hypot(d.b.x - d.a.x, d.b.y - d.a.y) < 3 * this.env.unit();
      if (!tiny) this.doc = apply(this.doc, { type: 'add', shapes: [d] });
    }
    this.draft = null;
    this.gesture = null;
  }

  /** 编辑框里的文字变化。 */
  setEditingText(text: string): void {
    if (this.editing) this.editing = { ...this.editing, shape: { ...this.editing.shape, text } };
  }

  /** 提交正在编辑的文字：空文字丢弃（重新编辑的已有文字被清空则删除），内容或样式有变化才记一步。 */
  commitText(): void {
    const ed = this.editing;
    if (!ed) return;
    this.editing = null;
    const text = ed.shape.text.replace(/\s+$/, '');
    if (!text) {
      if (ed.original) this.doc = apply(this.doc, { type: 'remove', shapes: [ed.original] });
      return;
    }
    const after: TextShape = { ...ed.shape, text, box: measureText(text, ed.shape.width) };
    if (!ed.original) {
      this.doc = apply(this.doc, { type: 'add', shapes: [after] });
      return;
    }
    const o = ed.original;
    if (o.text !== text || o.color !== after.color || o.width !== after.width) {
      this.doc = apply(this.doc, { type: 'replace', before: o, after });
    }
  }

  /** 鼠标指针形状。 */
  cursor(): string {
    return this.tool === 'eraser' ? 'cell' : 'crosshair';
  }
}
