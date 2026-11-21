// 取色：颜色值格式化。三种格式都是可以直接粘进 CSS 的写法。

export type ColorFormat = 'hex' | 'rgb' | 'hsl';
export const COLOR_FORMATS: ColorFormat[] = ['hex', 'rgb', 'hsl'];

export interface RGB {
  r: number;
  g: number;
  b: number;
}

const hex2 = (v: number) => v.toString(16).padStart(2, '0').toUpperCase();

export function toHex({ r, g, b }: RGB): string {
  return `#${hex2(r)}${hex2(g)}${hex2(b)}`;
}

/** RGB（0–255）→ HSL（色相 0–360，饱和度和亮度 0–100），均取整。 */
export function toHsl({ r, g, b }: RGB): { h: number; s: number; l: number } {
  const R = r / 255, G = g / 255, B = b / 255;
  const max = Math.max(R, G, B), min = Math.min(R, G, B);
  const l = (max + min) / 2;
  let h = 0, s = 0;
  if (max !== min) {
    const d = max - min;
    s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    if (max === R) h = (G - B) / d + (G < B ? 6 : 0);
    else if (max === G) h = (B - R) / d + 2;
    else h = (R - G) / d + 4;
    h *= 60;
  }
  return { h: Math.round(h) % 360, s: Math.round(s * 100), l: Math.round(l * 100) };
}

export function formatColor(c: RGB, f: ColorFormat): string {
  switch (f) {
    case 'hex': return toHex(c);
    case 'rgb': return `rgb(${c.r}, ${c.g}, ${c.b})`;
    case 'hsl': {
      const { h, s, l } = toHsl(c);
      return `hsl(${h}, ${s}%, ${l}%)`;
    }
  }
}

/** P 键：HEX → RGB → HSL → HEX。 */
export function nextFormat(f: ColorFormat): ColorFormat {
  return COLOR_FORMATS[(COLOR_FORMATS.indexOf(f) + 1) % COLOR_FORMATS.length];
}

/** W A S D 对应的光标位移（物理像素）。其他键返回 null。 */
export function wasdDelta(key: string): { x: number; y: number } | null {
  switch (key.toLowerCase()) {
    case 'w': return { x: 0, y: -1 };
    case 'a': return { x: -1, y: 0 };
    case 's': return { x: 0, y: 1 };
    case 'd': return { x: 1, y: 0 };
    default: return null;
  }
}

/**
 * 放大镜位置（CSS 像素）：默认在光标右下 22px，放不下就翻到左边 / 上边。
 * size 是放大镜自身尺寸，view 是窗口尺寸。
 */
export function magnifierPos(
  cursor: { x: number; y: number },
  size: { width: number; height: number },
  view: { width: number; height: number },
  gap = 22,
): { x: number; y: number } {
  let x = cursor.x + gap;
  let y = cursor.y + gap;
  if (x + size.width > view.width) x = cursor.x - gap - size.width;
  if (y + size.height > view.height) y = cursor.y - gap - size.height;
  return { x: Math.max(0, x), y: Math.max(0, y) };
}
