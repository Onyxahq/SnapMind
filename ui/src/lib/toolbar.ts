// 截图工具条：摆放位置与按钮定义。按钮的顺序、隐藏、「更多」在 M10.2 改为读配置。

import type { Rect } from './geometry';

/** 工具条上的动作。 */
export type ToolbarAction = 'pin' | 'save' | 'exit' | 'copy' | 'translate' | 'ocr' | 'markdown' | 'formula' | 'table' | 'long';

/**
 * 工具条位置（CSS 像素）：默认在选区下方、右对齐；下方放不下就放上方；
 * 上方也放不下（选区几乎占满整屏）就放进选区内的右下角。水平方向夹在屏幕内。
 */
export function placeToolbar(
  sel: Rect,
  bar: { width: number; height: number },
  view: { width: number; height: number },
  gap = 8,
  margin = 8,
): { x: number; y: number } {
  const x = Math.min(Math.max(sel.x + sel.width - bar.width, margin), view.width - bar.width - margin);
  const below = sel.y + sel.height + gap;
  if (below + bar.height <= view.height - margin) return { x, y: below };
  const above = sel.y - gap - bar.height;
  if (above >= margin) return { x, y: above };
  return { x, y: sel.y + sel.height - gap - bar.height };
}

/** 估算文件大小的显示：B / KB / MB。 */
export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10 * 1024 ? 1 : 0)} KB`;
  return `${(n / 1024 / 1024).toFixed(2)} MB`;
}

/** 锁定比例时，改了一边，另一边按原始比例跟着变。 */
export function lockedSize(
  orig: { width: number; height: number },
  changed: 'width' | 'height',
  value: number,
): { width: number; height: number } {
  const v = Math.max(1, Math.round(value));
  return changed === 'width'
    ? { width: v, height: Math.max(1, Math.round((v * orig.height) / orig.width)) }
    : { width: Math.max(1, Math.round((v * orig.width) / orig.height)), height: v };
}

/** 拼出保存路径的显示文本（Windows 分隔符）。 */
export function joinPath(dir: string, name: string, ext: string): string {
  const d = dir.replace(/[\\/]+$/, '');
  return `${d}\\${name}.${ext}`;
}
