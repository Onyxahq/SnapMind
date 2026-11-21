import { describe, expect, it } from 'vitest';
import type { MonitorGeom } from './geometry';
import { containsPoint, keepLevel, toLocalChain, wheelLevel } from './snap';

const RIGHT: MonitorGeom = { x: 1920, y: 0, width: 1920, height: 1080, scale: 1 };

describe('toLocalChain', () => {
  it('换成本屏坐标并裁到屏内', () => {
    const chain = [
      { x: 2000, y: 100, width: 50, height: 20 }, // 按钮
      { x: 1800, y: 50, width: 600, height: 400 }, // 跨两块屏的窗口
    ];
    expect(toLocalChain(chain, RIGHT)).toEqual([
      { x: 80, y: 100, width: 50, height: 20 },
      { x: 0, y: 50, width: 480, height: 400 },
    ]);
  });

  it('裁完为空或重复的去掉', () => {
    const chain = [
      { x: 0, y: 0, width: 100, height: 100 }, // 完全在左屏
      { x: 1920, y: 0, width: 1920, height: 1080 },
      { x: 1900, y: -10, width: 2000, height: 2000 }, // 裁完与上一个相同
    ];
    expect(toLocalChain(chain, RIGHT)).toEqual([{ x: 0, y: 0, width: 1920, height: 1080 }]);
  });
});

describe('层级', () => {
  const chain = [
    { x: 10, y: 10, width: 20, height: 10 },
    { x: 0, y: 0, width: 100, height: 100 },
    { x: 0, y: 0, width: 500, height: 500 },
  ];

  it('当前高亮的矩形仍在新链里就保持', () => {
    expect(keepLevel(chain, { x: 0, y: 0, width: 500, height: 500 })).toBe(2);
    expect(keepLevel(chain, { x: 1, y: 1, width: 5, height: 5 })).toBe(0);
    expect(keepLevel(chain, null)).toBe(0);
  });

  it('滚轮向上往外、向下往里，不越界', () => {
    expect(wheelLevel(0, -100, 3)).toBe(1);
    expect(wheelLevel(2, -100, 3)).toBe(2);
    expect(wheelLevel(1, 100, 3)).toBe(0);
    expect(wheelLevel(0, 100, 3)).toBe(0);
    expect(wheelLevel(5, 0, 0)).toBe(0);
  });

  it('点在矩形内（右下边不算）', () => {
    expect(containsPoint(chain[0], { x: 10, y: 10 })).toBe(true);
    expect(containsPoint(chain[0], { x: 30, y: 15 })).toBe(false);
  });
});
