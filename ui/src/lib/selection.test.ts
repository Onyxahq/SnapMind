import { describe, expect, it } from 'vitest';
import { cursorFor, fromCorners, hitTest, labelTop, move, nudgeDelta, resize } from './selection';

const W = 1920;
const H = 1080;
const R = { x: 100, y: 100, width: 200, height: 100 };

describe('fromCorners', () => {
  it('任意方向拖都得到正的宽高', () => {
    expect(fromCorners({ x: 300, y: 200 }, { x: 100, y: 50 }, W, H)).toEqual({ x: 100, y: 50, width: 200, height: 150 });
  });
  it('夹在显示器内', () => {
    expect(fromCorners({ x: -20, y: 10 }, { x: 5000, y: 2000 }, W, H)).toEqual({ x: 0, y: 10, width: W, height: H - 10 });
  });
});

describe('hitTest', () => {
  it('四角和四边中点的控制点', () => {
    expect(hitTest(R, { x: 100, y: 100 }, 6)).toBe('nw');
    expect(hitTest(R, { x: 203, y: 98 }, 6)).toBe('n');
    expect(hitTest(R, { x: 300, y: 200 }, 6)).toBe('se');
    expect(hitTest(R, { x: 100, y: 150 }, 6)).toBe('w');
  });
  it('边上任意位置都能拖', () => {
    expect(hitTest(R, { x: 130, y: 102 }, 6)).toBe('n');
    expect(hitTest(R, { x: 297, y: 120 }, 6)).toBe('e');
    expect(hitTest(R, { x: 260, y: 204 }, 6)).toBe('s');
  });
  it('内部是移动，外部是新建', () => {
    expect(hitTest(R, { x: 150, y: 130 }, 6)).toBe('move');
    expect(hitTest(R, { x: 50, y: 50 }, 6)).toBe(null);
    expect(hitTest(R, { x: 120, y: 120 + 200 }, 6)).toBe(null);
  });
  it('很小的选区也能抓到角', () => {
    const tiny = { x: 10, y: 10, width: 4, height: 4 };
    expect(hitTest(tiny, { x: 14, y: 14 }, 6)).not.toBe(null);
  });
});

describe('resize', () => {
  it('拖右下角', () => {
    expect(resize(R, 'se', 50, 20, W, H)).toEqual({ x: 100, y: 100, width: 250, height: 120 });
  });
  it('拖上边只改上边', () => {
    expect(resize(R, 'n', 30, -40, W, H)).toEqual({ x: 100, y: 60, width: 200, height: 140 });
  });
  it('拖过对边自动翻转', () => {
    // 左边往右拖 300：越过右边 100px
    expect(resize(R, 'w', 300, 0, W, H)).toEqual({ x: 300, y: 100, width: 100, height: 100 });
  });
  it('不超出显示器', () => {
    expect(resize(R, 'nw', -500, -500, W, H)).toEqual({ x: 0, y: 0, width: 300, height: 200 });
  });
});

describe('move / nudge', () => {
  it('移动保持尺寸并在边缘停住', () => {
    expect(move(R, 10, -5, W, H)).toEqual({ x: 110, y: 95, width: 200, height: 100 });
    expect(move(R, -1000, 5000, W, H)).toEqual({ x: 0, y: H - 100, width: 200, height: 100 });
  });
  it('方向键 1px，Shift 10px', () => {
    expect(nudgeDelta('ArrowLeft', false)).toEqual({ x: -1, y: 0 });
    expect(nudgeDelta('ArrowDown', true)).toEqual({ x: 0, y: 10 });
    expect(nudgeDelta('a', false)).toBe(null);
  });
});

describe('其他', () => {
  it('指针形状', () => {
    expect(cursorFor('ne')).toBe('nesw-resize');
    expect(cursorFor('move')).toBe('move');
    expect(cursorFor(null)).toBe('crosshair');
  });
  it('尺寸标签贴近顶端时放进选区内', () => {
    expect(labelTop(100)).toBe(72);
    expect(labelTop(20)).toBe(26);
  });
});
