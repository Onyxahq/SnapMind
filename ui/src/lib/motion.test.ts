import { describe, expect, it } from 'vitest';
import { cubicBezier, ease } from './motion';

describe('cubicBezier', () => {
  it('端点固定为 0 和 1', () => {
    const f = cubicBezier(0.4, 0, 1, 1);
    expect(f(0)).toBe(0);
    expect(f(1)).toBe(1);
    expect(f(-1)).toBe(0);
    expect(f(2)).toBe(1);
  });

  it('线性曲线等于自身', () => {
    const f = cubicBezier(0.25, 0.25, 0.75, 0.75);
    for (const x of [0.1, 0.33, 0.5, 0.9]) expect(f(x)).toBeCloseTo(x, 4);
  });

  it('与 CSS ease (0.25,0.1,0.25,1) 的已知值一致', () => {
    const f = cubicBezier(0.25, 0.1, 0.25, 1);
    expect(f(0.5)).toBeCloseTo(0.8024, 3);
  });

  it('出现曲线有过冲，开关曲线过冲更大', () => {
    const max = (f: (x: number) => number) =>
      Math.max(...Array.from({ length: 101 }, (_, i) => f(i / 100)));
    expect(max(ease.in)).toBeGreaterThan(1);
    expect(max(ease.switch)).toBeGreaterThan(max(ease.in));
  });
});
