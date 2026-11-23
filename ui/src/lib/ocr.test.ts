import { describe, expect, it } from 'vitest';
import { allLines, bounds, hitLine, newOcrCard, ocrMeta, paragraphBoxes, progressLabel, toggleAll, toggleLine } from './ocr';
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

  it('formats status text', () => {
    expect(progressLabel({ installed: false, tableInstalled: false, downloading: true, received: 8_703_180, total: 21_510_548, error: null })).toBe(
      '8.3 / 20.5 MB',
    );
    expect(progressLabel({ installed: false, tableInstalled: false, downloading: true, received: 0, total: 21_510_548, error: null, source: '魔搭 ModelScope' })).toBe(
      '0.0 / 20.5 MB · 魔搭 ModelScope',
    );
    const c = { ...newOcrCard('x', { x: 0, y: 0, width: 1, height: 1 }), stage: 'done' as const, result, pick: [0, 1, 2] };
    expect(ocrMeta(c)).toBe('3 行 · 2 段 · 本地 OCR 0.21 s');
    expect(ocrMeta({ ...c, pick: [0] })).toBe('已选 1 / 3 行');
  });

  it('clicks boxes to toggle lines and Ctrl+A toggles all', () => {
    expect(hitLine(result, { x: 50, y: 20 })).toBe(0);
    expect(hitLine(result, { x: 50, y: 40 })).toBe(1);
    expect(hitLine(result, { x: 50, y: 60 })).toBe(-1);
    expect(hitLine(result, { x: 50, y: 57 }, 4)).toBe(1); // 外扩的边上也算
    expect(toggleLine(allLines(result), 1)).toEqual([0, 2]);
    expect(toggleLine([2], 0)).toEqual([0, 2]);
    expect(toggleLine([0], 0)).toEqual([]);
    // 全选时 Ctrl+A 是全不选，其他情况是全选
    expect(toggleAll(result, [0, 1, 2])).toEqual([]);
    expect(toggleAll(result, [1])).toEqual([0, 1, 2]);
    expect(toggleAll(result, [])).toEqual([0, 1, 2]);
  });
});
