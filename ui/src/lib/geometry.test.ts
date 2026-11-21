import { describe, expect, it } from 'vitest';
import {
  cssSize,
  cssToFramePixel,
  cssToPhysical,
  monitorAt,
  physicalToCss,
  type MonitorGeom,
} from './geometry';

// 典型混合布局：主屏 4K@150%，右侧 1080p@100%，左侧竖放 1440p@125%（坐标为负）
const MAIN: MonitorGeom = { x: 0, y: 0, width: 3840, height: 2160, scale: 1.5 };
const RIGHT: MonitorGeom = { x: 3840, y: 540, width: 1920, height: 1080, scale: 1 };
const LEFT: MonitorGeom = { x: -1440, y: -200, width: 1440, height: 2560, scale: 1.25 };
const ALL = [MAIN, RIGHT, LEFT];

describe('坐标换算', () => {
  it('视口 CSS 尺寸 = 物理尺寸 / 缩放', () => {
    expect(cssSize(MAIN)).toEqual({ width: 2560, height: 1440 });
    expect(cssSize(LEFT)).toEqual({ width: 1152, height: 2048 });
  });

  it('CSS → 物理：乘缩放再加显示器原点', () => {
    expect(cssToPhysical(MAIN, { x: 100, y: 10 })).toEqual({ x: 150, y: 15 });
    expect(cssToPhysical(RIGHT, { x: 0, y: 0 })).toEqual({ x: 3840, y: 540 });
    expect(cssToPhysical(LEFT, { x: 8, y: 8 })).toEqual({ x: -1430, y: -190 });
  });

  it('物理 → CSS 与 CSS → 物理互逆', () => {
    for (const m of ALL) {
      const css = { x: 123.2, y: 45.6 };
      const back = physicalToCss(m, cssToPhysical(m, css));
      expect(back.x).toBeCloseTo(css.x, 0);
      expect(back.y).toBeCloseTo(css.y, 0);
    }
  });

  it('每个显示器的右下角 CSS 坐标都落在自己的最后一个物理像素上', () => {
    for (const m of ALL) {
      const { width, height } = cssSize(m);
      const px = cssToFramePixel(m, { x: width - 0.01, y: height - 0.01 });
      expect(px).toEqual({ x: m.width - 1, y: m.height - 1 });
    }
  });

  it('截图像素坐标夹在图内', () => {
    expect(cssToFramePixel(MAIN, { x: -5, y: 99999 })).toEqual({ x: 0, y: 2159 });
  });

  it('按物理坐标找显示器，含负坐标与缝隙', () => {
    expect(monitorAt(ALL, { x: 10, y: 10 })).toBe(0);
    expect(monitorAt(ALL, { x: 3840, y: 540 })).toBe(1);
    expect(monitorAt(ALL, { x: 3840, y: 539 })).toBe(-1); // 右屏上方的空白
    expect(monitorAt(ALL, { x: -1, y: -200 })).toBe(2);
    expect(monitorAt(ALL, { x: 0, y: -1 })).toBe(-1);
  });
});
