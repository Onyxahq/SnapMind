import { describe, expect, it } from 'vitest';
import { ACCENTS, resolveTheme } from './theme';

describe('resolveTheme', () => {
  it('跟随系统时取系统明暗', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
  });
  it('手动指定时忽略系统', () => {
    expect(resolveTheme('light', true)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
  });
});

describe('ACCENTS', () => {
  it('四种强调色，顺序与设置页一致', () => {
    expect(ACCENTS.map((a) => a.id)).toEqual(['blue', 'sky', 'violet', 'green']);
  });
});
