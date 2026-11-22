// 工具条：摆放位置、按钮目录，以及按配置排出来的按钮（M10.2：顺序、隐藏、收进「更多」）。
// 截图工具条和贴图工具条各有一份布局（config.toolbar.capture / pin），都由这里解析。

import type { Rect } from './geometry';
import type { IconName } from './ui/Icon.svelte';
import type { ToolbarLayout } from './ipc';
import type { Tool } from './annotate';

/** 布局里的分隔线：它后面的按钮收进「⋯ 更多」。 */
export const MORE = 'more';

/** 工具条上可以排的一个按钮。tool：标注工具；undo：撤销；action：其他动作。 */
export interface BarItem {
  id: string;
  label: string;
  icon: IconName;
  key?: string;
  kind: 'tool' | 'undo' | 'action';
}

/** 截图工具条的全部按钮（退出、复制固定在最右边，不在这里）。 */
export const CAPTURE_ITEMS: BarItem[] = [
  { id: 'rect', label: '矩形 / 椭圆', icon: 'rect', key: '1', kind: 'tool' },
  { id: 'pen', label: '画笔 / 荧光笔', icon: 'pen', key: '2', kind: 'tool' },
  { id: 'arrow', label: '箭头 / 直线', icon: 'arrow', key: '3', kind: 'tool' },
  { id: 'text', label: '文字', icon: 'textT', key: '4', kind: 'tool' },
  { id: 'number', label: '序号', icon: 'number', key: '5', kind: 'tool' },
  { id: 'mosaic', label: '马赛克 / 模糊', icon: 'mosaic', key: '6', kind: 'tool' },
  { id: 'eraser', label: '橡皮擦', icon: 'eraser', key: '7', kind: 'tool' },
  { id: 'undo', label: '撤销', icon: 'undo', key: 'Ctrl+Z', kind: 'undo' },
  { id: 'long', label: '长截图', icon: 'long', key: 'L', kind: 'action' },
  { id: 'record', label: '录屏', icon: 'video', key: 'R', kind: 'action' },
  { id: 'ocr', label: '识别文字', icon: 'scan', key: 'Shift+C', kind: 'action' },
  { id: 'markdown', label: '识别为 Markdown', icon: 'markdown', key: 'Shift+Alt+R', kind: 'action' },
  { id: 'translate', label: '翻译', icon: 'translate', key: 'Ctrl+Q', kind: 'action' },
  { id: 'formula', label: '公式识别', icon: 'formula', kind: 'action' },
  { id: 'table', label: '表格识别', icon: 'table', kind: 'action' },
  { id: 'pin', label: '贴图', icon: 'pin', key: 'Ctrl+T', kind: 'action' },
  { id: 'save', label: '保存…', icon: 'download', key: 'Ctrl+S', kind: 'action' },
];

/** 贴图工具条的按钮（关闭固定在最右边；不透明度滑块固定在「更多」里）。 */
export const PIN_ITEMS: BarItem[] = [
  { id: 'annotate', label: '标注', icon: 'pen', key: '空格', kind: 'action' },
  { id: 'save', label: '保存…', icon: 'download', key: 'Ctrl+S', kind: 'action' },
  { id: 'copy', label: '复制', icon: 'copy', key: 'Ctrl+C', kind: 'action' },
  { id: 'through', label: '鼠标穿透', icon: 'through', key: 'Ctrl+Shift+X', kind: 'action' },
  { id: 'zoom100', label: '缩放到 100%', icon: 'zoom100', key: 'Ctrl+0', kind: 'action' },
];

/** 布局里的一行：按钮，或「更多」分隔线。 */
export type LayoutRow = { item: BarItem; hidden: boolean } | { divider: true };

/**
 * 把配置里的布局整理成完整的一列：只保留认识的按钮、去掉重复；目录里有但布局里没有的
 * （新版本新加的按钮）插到分隔线前面；没有分隔线时补在最后。
 */
export function layoutRows(layout: ToolbarLayout, catalog: BarItem[]): LayoutRow[] {
  const byId = new Map(catalog.map((i) => [i.id, i]));
  const seen = new Set<string>();
  const rows: LayoutRow[] = [];
  let divider = false;
  for (const id of layout.order) {
    if (id === MORE) {
      if (!divider) rows.push({ divider: true });
      divider = true;
      continue;
    }
    const item = byId.get(id);
    if (!item || seen.has(id)) continue;
    seen.add(id);
    rows.push({ item, hidden: layout.hidden.includes(id) });
  }
  if (!divider) rows.push({ divider: true });
  const missing = catalog.filter((i) => !seen.has(i.id)).map((item) => ({ item, hidden: false }));
  const at = rows.findIndex((r) => 'divider' in r);
  rows.splice(at, 0, ...missing);
  return rows;
}

/** 一列布局 → 存进配置的样子。 */
export function toLayout(rows: LayoutRow[]): ToolbarLayout {
  return {
    order: rows.map((r) => ('divider' in r ? MORE : r.item.id)),
    hidden: rows.filter((r): r is { item: BarItem; hidden: boolean } => !('divider' in r) && r.hidden).map((r) => r.item.id),
  };
}

/** 工具条上实际显示的按钮：分隔线前面的在条上，后面的在「更多」里；隐藏的不显示。 */
export function visibleBar(layout: ToolbarLayout, catalog: BarItem[]): { main: BarItem[]; more: BarItem[] } {
  const main: BarItem[] = [];
  const more: BarItem[] = [];
  let inMore = false;
  for (const r of layoutRows(layout, catalog)) {
    if ('divider' in r) inMore = true;
    else if (!r.hidden) (inMore ? more : main).push(r.item);
  }
  return { main, more };
}

/** 是不是标注工具的 id。 */
export const isTool = (item: BarItem): item is BarItem & { id: Tool } => item.kind === 'tool';

/** 工具条上的动作。 */
export type ToolbarAction = 'pin' | 'save' | 'exit' | 'copy' | 'translate' | 'ocr' | 'markdown' | 'formula' | 'table' | 'long' | 'record';

/** 截图工具条按钮 id → 动作（标注工具和撤销另外处理）。 */
export const asAction = (id: string): ToolbarAction => id as ToolbarAction;

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
