import { describe, expect, it } from 'vitest';
import { formatColor, magnifierPos, nextFormat, toHex, toHsl, wasdDelta } from './color';

describe('颜色格式', () => {
  it('HEX 大写两位', () => {
    expect(toHex({ r: 37, g: 99, b: 235 })).toBe('#2563EB');
    expect(toHex({ r: 0, g: 5, b: 255 })).toBe('#0005FF');
  });

  it('HSL 与常见颜色一致', () => {
    expect(toHsl({ r: 255, g: 0, b: 0 })).toEqual({ h: 0, s: 100, l: 50 });
    expect(toHsl({ r: 0, g: 0, b: 255 })).toEqual({ h: 240, s: 100, l: 50 });
    expect(toHsl({ r: 128, g: 128, b: 128 })).toEqual({ h: 0, s: 0, l: 50 });
    expect(toHsl({ r: 37, g: 99, b: 235 })).toEqual({ h: 221, s: 83, l: 53 });
  });

  it('三种格式都是 CSS 写法', () => {
    const c = { r: 37, g: 99, b: 235 };
    expect(formatColor(c, 'hex')).toBe('#2563EB');
    expect(formatColor(c, 'rgb')).toBe('rgb(37, 99, 235)');
    expect(formatColor(c, 'hsl')).toBe('hsl(221, 83%, 53%)');
  });

  it('P 键循环', () => {
    expect(nextFormat('hex')).toBe('rgb');
    expect(nextFormat('rgb')).toBe('hsl');
    expect(nextFormat('hsl')).toBe('hex');
  });
});

describe('光标与放大镜', () => {
  it('WASD 逐像素，大小写都认', () => {
    expect(wasdDelta('w')).toEqual({ x: 0, y: -1 });
    expect(wasdDelta('D')).toEqual({ x: 1, y: 0 });
    expect(wasdDelta('q')).toBe(null);
  });

  it('放大镜默认在右下，靠边时翻转', () => {
    const size = { width: 156, height: 160 };
    const view = { width: 1000, height: 600 };
    expect(magnifierPos({ x: 100, y: 100 }, size, view)).toEqual({ x: 122, y: 122 });
    expect(magnifierPos({ x: 900, y: 500 }, size, view)).toEqual({ x: 722, y: 318 });
  });
});
