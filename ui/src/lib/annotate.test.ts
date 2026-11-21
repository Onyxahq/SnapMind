import { describe, expect, it } from 'vitest';
import {
  addPoint,
  apply,
  arrowHead,
  constrain,
  emptyDoc,
  hitShape,
  nextNumber,
  textAt,
  toolForKey,
  undo,
  type Shape,
} from './annotate';

const rect = (id: number): Shape => ({ id, kind: 'rect', ellipse: false, color: '#EF4444', width: 4, a: { x: 10, y: 10 }, b: { x: 110, y: 60 } });
const num = (id: number, n: number): Shape => ({ id, kind: 'number', n, color: '#EF4444', width: 4, at: { x: 50, y: 50 } });

describe('快捷键', () => {
  it('1–7 依次对应七个工具', () => {
    expect(toolForKey('1')).toBe('rect');
    expect(toolForKey('3')).toBe('arrow');
    expect(toolForKey('4')).toBe('text');
    expect(toolForKey('6')).toBe('mosaic');
    expect(toolForKey('7')).toBe('eraser');
    expect(toolForKey('8')).toBe(null);
    expect(toolForKey('a')).toBe(null);
  });
});

describe('Shift 约束', () => {
  it('矩形变正方形，方向保持', () => {
    expect(constrain('rect', { x: 0, y: 0 }, { x: 30, y: -10 })).toEqual({ x: 30, y: -30 });
  });
  it('直线吸附到 45° 倍数', () => {
    expect(constrain('arrow', { x: 0, y: 0 }, { x: 100, y: 8 })).toEqual({ x: 100, y: 0 });
    const p = constrain('arrow', { x: 0, y: 0 }, { x: 100, y: 90 });
    expect(p.x).toBe(p.y);
  });
});

describe('几何', () => {
  it('箭头尖端在终点，两翼对称', () => {
    const [tip, l, r] = arrowHead({ x: 0, y: 0 }, { x: 100, y: 0 }, 4);
    expect(tip).toEqual({ x: 100, y: 0 });
    expect(l.x).toBeCloseTo(r.x);
    expect(l.y).toBeCloseTo(-r.y);
    expect(l.x).toBeLessThan(100);
  });
  it('画笔点太密时丢弃', () => {
    let pts = addPoint([], { x: 0, y: 0 });
    pts = addPoint(pts, { x: 1, y: 0 });
    pts = addPoint(pts, { x: 3, y: 0 });
    expect(pts).toEqual([{ x: 0, y: 0 }, { x: 3, y: 0 }]);
  });
  it('序号取最大值 + 1', () => {
    expect(nextNumber([])).toBe(1);
    expect(nextNumber([num(1, 1), rect(2), num(3, 4)])).toBe(5);
  });
});

describe('橡皮擦命中', () => {
  it('矩形只算边框', () => {
    expect(hitShape(rect(1), { x: 12, y: 30 }, 4)).toBe(true);
    expect(hitShape(rect(1), { x: 60, y: 35 }, 4)).toBe(false);
  });
  it('椭圆边框', () => {
    const e: Shape = { ...(rect(1) as Extract<Shape, { kind: 'rect' }>), ellipse: true };
    expect(hitShape(e, { x: 60, y: 10 }, 4)).toBe(true);
    expect(hitShape(e, { x: 60, y: 35 }, 4)).toBe(false);
  });
  it('箭头、画笔、序号', () => {
    const a: Shape = { id: 1, kind: 'arrow', line: false, color: '#000', width: 4, a: { x: 0, y: 0 }, b: { x: 100, y: 0 } };
    expect(hitShape(a, { x: 50, y: 3 }, 4)).toBe(true);
    expect(hitShape(a, { x: 50, y: 20 }, 4)).toBe(false);
    const p: Shape = { id: 2, kind: 'pen', highlighter: false, color: '#000', width: 4, points: [{ x: 0, y: 0 }, { x: 0, y: 100 }] };
    expect(hitShape(p, { x: 2, y: 60 }, 4)).toBe(true);
    expect(hitShape(num(3, 1), { x: 60, y: 55 }, 4)).toBe(true);
  });
});

describe('文字与马赛克命中', () => {
  const t: Shape = { id: 1, kind: 'text', text: 'hi', color: '#000', width: 22, at: { x: 100, y: 100 }, box: { w: 40, h: 29 } };
  const m: Shape = { id: 2, kind: 'mosaic', blur: false, color: '', width: 10, a: { x: 0, y: 0 }, b: { x: 50, y: 50 } };
  it('文字按量出的框命中', () => {
    expect(hitShape(t, { x: 120, y: 110 }, 2)).toBe(true);
    expect(hitShape(t, { x: 150, y: 110 }, 2)).toBe(false);
    expect(textAt([m, t], { x: 120, y: 110 }, 2)).toBe(t);
    expect(textAt([m, t], { x: 20, y: 20 }, 2)).toBe(null);
  });
  it('马赛克整块都算', () => {
    expect(hitShape(m, { x: 25, y: 25 }, 2)).toBe(true);
    expect(hitShape(m, { x: 80, y: 25 }, 2)).toBe(false);
  });
});

describe('撤销历史', () => {
  it('添加、删除、逐步撤销', () => {
    let d = emptyDoc();
    d = apply(d, { type: 'add', shapes: [rect(1)] });
    d = apply(d, { type: 'add', shapes: [num(2, 1)] });
    d = apply(d, { type: 'remove', shapes: [d.shapes[0]] });
    expect(d.shapes.map((s) => s.id)).toEqual([2]);
    d = undo(d);
    expect(d.shapes.map((s) => s.id)).toEqual([1, 2]);
    d = undo(d);
    expect(d.shapes.map((s) => s.id)).toEqual([1]);
    d = undo(d);
    d = undo(d);
    expect(d).toEqual(emptyDoc());
  });
  it('替换（改文字 / 拖动文字）可以撤销', () => {
    const t1: Shape = { id: 5, kind: 'text', text: 'hi', color: '#000', width: 22, at: { x: 0, y: 0 }, box: { w: 20, h: 29 } };
    const t2: Shape = { ...t1, text: 'hello', at: { x: 30, y: 40 } };
    let d = apply(emptyDoc(), { type: 'add', shapes: [rect(1), t1] });
    d = apply(d, { type: 'replace', before: t1, after: t2 });
    expect(d.shapes[1]).toBe(t2);
    d = undo(d);
    expect(d.shapes[1]).toBe(t1);
    expect(d.history).toHaveLength(1);
  });

  it('空操作不进历史', () => {
    expect(apply(emptyDoc(), { type: 'remove', shapes: [] }).history).toHaveLength(0);
  });
});
