import { describe, expect, it } from 'vitest';
import { hintsFor, nearPanel, placeHints } from './hints';

const VIEW = { width: 1600, height: 900 };
const PANEL = { width: 280, height: 200 };

describe('hintsFor', () => {
  it('两种状态都以 Esc 退出结尾，内容不同', () => {
    for (const s of ['idle', 'selected'] as const) {
      const h = hintsFor(s);
      expect(h.at(-1)).toEqual({ k: 'Esc', label: '退出' });
    }
    expect(hintsFor('idle')[0].label).toBe('框选区域');
    expect(hintsFor('selected')[0].label).toBe('调整大小');
  });
});

describe('placeHints', () => {
  it('没有选区时在左下角', () => {
    expect(placeHints(PANEL, VIEW, null)).toEqual({ x: 16, y: 684 });
  });

  it('左下被挡时换到左上', () => {
    const sel = { x: 0, y: 600, width: 400, height: 300 };
    expect(placeHints(PANEL, VIEW, sel)).toEqual({ x: 16, y: 16 });
  });

  it('左边整列被挡时换到右下', () => {
    const sel = { x: 0, y: 0, width: 400, height: 900 };
    expect(placeHints(PANEL, VIEW, sel)).toEqual({ x: 1304, y: 684 });
  });

  it('四角都被挡（全屏选区）时取重叠最小的，即仍在左下', () => {
    const sel = { x: 0, y: 0, width: 1600, height: 900 };
    expect(placeHints(PANEL, VIEW, sel)).toEqual({ x: 16, y: 684 });
  });

  it('选区只挡住一部分时取重叠更少的角', () => {
    // 左下、左上、右下都被大面积挡住，右上只挡一点
    const sel = { x: 0, y: 100, width: 1500, height: 800 };
    expect(placeHints(PANEL, VIEW, sel)).toEqual({ x: 1304, y: 16 });
  });
});

describe('nearPanel', () => {
  it('光标靠近面板 24px 内算靠近', () => {
    const pos = { x: 16, y: 684 };
    expect(nearPanel({ x: 100, y: 700 }, pos, PANEL)).toBe(true);
    expect(nearPanel({ x: 310, y: 670 }, pos, PANEL)).toBe(true);
    expect(nearPanel({ x: 400, y: 500 }, pos, PANEL)).toBe(false);
    expect(nearPanel(null, pos, PANEL)).toBe(false);
  });
});
