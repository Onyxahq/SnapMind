import { describe, expect, it } from 'vitest';
import { opacityStep, zoomFactor } from './pin';

describe('贴图', () => {
  it('滚轮缩放倍数互为倒数', () => {
    expect(zoomFactor(-100)).toBeCloseTo(1.1);
    expect(zoomFactor(100) * zoomFactor(-100)).toBeCloseTo(1);
  });

  it('不透明度每格 10%，夹在 10%–100%', () => {
    expect(opacityStep(1, 100)).toBe(0.9);
    expect(opacityStep(0.9, -100)).toBe(1);
    expect(opacityStep(1, -100)).toBe(1);
    expect(opacityStep(0.1, 100)).toBe(0.1);
    expect(opacityStep(0.3, 100)).toBe(0.2);
  });
});
