import { describe, expect, it } from 'vitest';
import { Annotator } from './annotator.svelte';

/** 画布 1000×1000、1 CSS 像素 = 2 画布像素，可绘制区域 [100,900] */
const make = () =>
  new Annotator({
    unit: () => 2,
    clamp: (p) => ({ x: Math.min(Math.max(p.x, 100), 900), y: Math.min(Math.max(p.y, 100), 900) }),
  });

describe('Annotator', () => {
  it('不在标注模式时按下不处理', () => {
    const a = make();
    expect(a.down({ x: 200, y: 200 }, false)).toBe(false);
    expect(a.hasShapes).toBe(false);
  });

  it('画矩形：线宽按 unit 换算，点夹进可绘制区域，Shift 约束正方形', () => {
    const a = make();
    a.setTool('rect');
    expect(a.down({ x: 50, y: 200 }, false)).toBe(true);
    a.move({ x: 300, y: 260 }, true);
    a.up();
    const s = a.doc.shapes[0];
    expect(s.kind).toBe('rect');
    if (s.kind !== 'rect') return;
    expect(s.a).toEqual({ x: 100, y: 200 });
    expect(s.b).toEqual({ x: 300, y: 400 });
    expect(s.width).toBe(8); // 中档 4px × 2
  });

  it('误触的极短形状不保留', () => {
    const a = make();
    a.setTool('arrow');
    a.down({ x: 200, y: 200 }, false);
    a.move({ x: 202, y: 201 }, false);
    a.up();
    expect(a.hasShapes).toBe(false);
  });

  it('序号单击放下并递增，撤销回退', () => {
    const a = make();
    a.setTool('number');
    expect(a.down({ x: 200, y: 200 }, false)).toBe(false);
    a.down({ x: 300, y: 300 }, false);
    expect(a.doc.shapes.map((s) => (s.kind === 'number' ? s.n : 0))).toEqual([1, 2]);
    a.undo();
    expect(a.doc.shapes).toHaveLength(1);
    expect(a.canUndo).toBe(true);
  });

  it('橡皮擦一笔划过的形状松手后一次删除，划过时先隐藏', () => {
    const a = make();
    a.setTool('number');
    a.down({ x: 200, y: 200 }, false);
    a.down({ x: 600, y: 600 }, false);
    a.setTool('eraser');
    a.down({ x: 200, y: 200 }, false);
    a.move({ x: 600, y: 600 }, false);
    expect(a.visible).toHaveLength(0);
    expect(a.doc.shapes).toHaveLength(2);
    a.up();
    expect(a.doc.shapes).toHaveLength(0);
    a.undo();
    expect(a.doc.shapes).toHaveLength(2);
  });

  it('改样式只影响对应工具；reset 清空标注但保留样式', () => {
    const a = make();
    a.setOpts('rect', { color: 3 });
    expect(a.opts.rect.color).toBe(3);
    expect(a.opts.pen.color).toBe(0);
    a.setTool('number');
    a.down({ x: 200, y: 200 }, false);
    a.reset();
    expect(a.hasShapes).toBe(false);
    expect(a.tool).toBe(null);
    expect(a.opts.rect.color).toBe(3);
  });
});
