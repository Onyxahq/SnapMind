import { describe, expect, it } from 'vitest';
import { bounds, newOcrCard, ocrMeta, paragraphBoxes, pickLines, progressLabel } from './ocr';
import type { OcrResult } from './ipc';

const result: OcrResult = {
  lines: [
    { x: 10, y: 10, width: 200, height: 20, text: 'a' },
    { x: 10, y: 34, width: 150, height: 20, text: 'b' },
    { x: 10, y: 80, width: 120, height: 20, text: 'c' },
  ],
  paragraphs: [[0, 1], [2]],
  text: 'a b\n\nc',
  ms: 210,
};

describe('ocr card helpers', () => {
  it('builds paragraph boxes from line boxes', () => {
    expect(bounds([{ x: 5, y: 5, width: 10, height: 10 }, { x: 0, y: 12, width: 4, height: 8 }])).toEqual({
      x: 0,
      y: 5,
      width: 15,
      height: 15,
    });
    expect(paragraphBoxes(result)).toEqual([
      { x: 10, y: 10, width: 200, height: 44 },
      { x: 10, y: 80, width: 120, height: 20 },
    ]);
    // 只画选中的行；一行都没选的段落不画
    expect(paragraphBoxes(result, new Set([1]))).toEqual([{ x: 10, y: 34, width: 150, height: 20 }]);
  });

  it('picks lines touched by the right-drag rectangle', () => {
    expect(pickLines(result, { x: 0, y: 40, width: 30, height: 50 })).toEqual([1, 2]);
    expect(pickLines(result, { x: 300, y: 0, width: 10, height: 100 })).toEqual([]);
    // 只擦到边缘（不重叠）不算
    expect(pickLines(result, { x: 0, y: 54, width: 400, height: 26 })).toEqual([]);
  });

  it('formats status text', () => {
    expect(progressLabel({ installed: false, downloading: true, received: 8_703_180, total: 21_510_548, error: null })).toBe(
      '8.3 / 20.5 MB',
    );
    const c = { ...newOcrCard('x', { x: 0, y: 0, width: 1, height: 1 }), stage: 'done' as const, result };
    expect(ocrMeta(c)).toBe('3 行 · 2 段 · 本地 OCR 0.21 s');
    expect(ocrMeta({ ...c, pick: [0] })).toBe('已选 1 / 3 行 · 右键单击恢复全部');
  });
});
