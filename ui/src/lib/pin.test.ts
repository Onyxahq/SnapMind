import { describe, expect, it } from 'vitest';
import { opacityStep, wrapText, zoomFactor } from './pin';

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

describe('文字卡片换行', () => {
  // 每个字符宽 1
  const m = (s: string) => Array.from(s).length;

  it('拉丁文在空格处断开', () => {
    expect(wrapText('hello world foo', 11, m)).toEqual(['hello world', 'foo']);
    expect(wrapText('hello world', 7, m)).toEqual(['hello', 'world']);
  });
  it('中文逐字断开', () => {
    expect(wrapText('一二三四五六七', 3, m)).toEqual(['一二三', '四五六', '七']);
  });
  it('保留空行和段落', () => {
    expect(wrapText('a\n\nb', 10, m)).toEqual(['a', '', 'b']);
  });
  it('超长单词按字断开', () => {
    expect(wrapText('abcdefgh', 3, m)).toEqual(['abc', 'def', 'gh']);
  });
});
