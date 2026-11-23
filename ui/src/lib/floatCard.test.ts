import { describe, expect, it } from 'vitest';
import { clampBox, edgeCursor, fitPlace, hitEdges, resizeBox } from './floatCard';

const view = { width: 1920, height: 1080 };

describe('floatCard', () => {
  it('hitEdges: 边、角、中间', () => {
    expect(hitEdges(200, 150, 400, 300)).toBeNull();
    expect(hitEdges(2, 150, 400, 300)).toEqual({ n: false, s: false, w: true, e: false });
    expect(hitEdges(398, 296, 400, 300)).toEqual({ n: false, s: true, w: false, e: true });
    // 贴着下边、离右边 10px（在角区里）也算拉右下角
    expect(hitEdges(390, 297, 400, 300)).toEqual({ n: false, s: true, w: false, e: true });
    expect(edgeCursor(hitEdges(390, 297, 400, 300))).toBe('nwse-resize');
    expect(edgeCursor(hitEdges(2, 297, 400, 300))).toBe('nesw-resize');
    expect(edgeCursor(hitEdges(200, 2, 400, 300))).toBe('ns-resize');
    expect(edgeCursor(null)).toBe('');
    // 圆角里面一点（离两边各 8px）也算拉角
    expect(hitEdges(392, 292, 400, 300)).toEqual({ n: false, s: true, w: false, e: true });
    expect(hitEdges(8, 8, 400, 300)).toEqual({ n: true, s: false, w: true, e: false });
  });

  it('resizeBox: 对边不动、有最小尺寸、不出屏幕', () => {
    const start = { x: 100, y: 100, w: 400, h: 300 };
    const min = { w: 280, h: 160 };
    expect(resizeBox(start, { n: false, s: true, w: false, e: true }, 50, 40, min, view)).toEqual({ x: 100, y: 100, w: 450, h: 340 });
    expect(resizeBox(start, { n: false, s: false, w: true, e: false }, -30, 0, min, view)).toEqual({ x: 70, y: 100, w: 430, h: 300 });
    expect(resizeBox(start, { n: false, s: false, w: true, e: false }, 300, 0, min, view)).toEqual({ x: 220, y: 100, w: 280, h: 300 });
    expect(resizeBox(start, { n: true, s: false, w: false, e: false }, 0, -500, min, view)).toEqual({ x: 100, y: 8, w: 400, h: 392 });
    expect(resizeBox(start, { n: false, s: false, w: false, e: true }, 5000, 0, min, view).w).toBe(1920 - 8 - 100);
  });

  it('fitPlace: 优先原地长、不挡选区', () => {
    const sel = { x: 800, y: 300, width: 300, height: 200 };
    // 在右边：原地往右长
    expect(fitPlace({ x: 1112, y: 300, w: 380, h: 300 }, 640, 500, sel, view)).toEqual({ x: 1112, y: 300 });
    // 在左边：贴着右边往左长
    expect(fitPlace({ x: 408, y: 300, w: 380, h: 300 }, 640, 500, sel, view)).toEqual({ x: 148, y: 300 });
    // 右边不够宽、原地会挡住：换到左边
    const r = { x: 1200, y: 300, width: 300, height: 200 };
    expect(fitPlace({ x: 1512, y: 300, w: 380, h: 300 }, 640, 500, r, view)).toEqual({ x: 1200 - 12 - 640, y: 300 });
    // 本来就压着选区：不换地方
    expect(fitPlace({ x: 700, y: 300, w: 380, h: 300 }, 640, 500, sel, view)).toEqual({ x: 700, y: 300 });
  });

  it('clampBox: 收进屏幕', () => {
    expect(clampBox(-50, 2000, 400, 300, view)).toEqual({ x: 8, y: 1080 - 300 - 8 });
    expect(clampBox(300, 200, 400, 300, view)).toEqual({ x: 300, y: 200 });
  });
});
