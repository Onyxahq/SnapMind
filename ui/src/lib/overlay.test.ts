import { describe, expect, it } from 'vitest';
import { drawable, fitText, MIN_SIZE, type OverlayBlock } from './overlay';

/** 每个字宽 = 字号 × 0.5（等宽假设，便于算） */
const measure = (s: string, size: number) => s.length * size * 0.5;

const block = (over: Partial<OverlayBlock>): OverlayBlock => ({
  x: 0,
  y: 0,
  width: 208,
  height: 28,
  fg: [0, 0, 0],
  bg: [255, 255, 255],
  size: 19,
  keep: false,
  src: 'Original text',
  dst: '译文',
  ...over,
});

describe('overlay fitting', () => {
  it('keeps the original size when the translation fits on one line', () => {
    const f = fitText(block({ dst: 'short' }), measure);
    expect(f.size).toBe(20); // 19 / 0.95
    expect(f.lines).toEqual(['short']);
    expect(f.dx).toBe(5); // ceil(19 × 0.25)
  });

  it('shrinks a long translation until it fits the block height', () => {
    const b = block({ height: 60, dst: 'a much longer translated sentence that needs wrapping' });
    const f = fitText(b, measure);
    expect(f.size).toBeLessThan(20);
    expect(f.lines.length).toBeGreaterThan(1);
    expect(f.size * (f.lines.length * 1.25 - 0.25)).toBeLessThanOrEqual(60 - 5 + 0.001);
    expect(f.lines.every((l) => measure(l, f.size) <= 198 + 0.001)).toBe(true);
  });

  it('stops at the minimum size instead of looping', () => {
    const f = fitText(block({ height: 12, width: 40, dst: 'x'.repeat(400) }), measure);
    expect(f.size).toBe(MIN_SIZE);
  });

  it('skips kept, empty and unchanged blocks', () => {
    const bs = [block({}), block({ keep: true }), block({ dst: ' ' }), block({ src: 'Same', dst: 'Same' })];
    expect(drawable(bs)).toHaveLength(1);
  });
});
