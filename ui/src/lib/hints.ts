// 截图编辑态的操作提示：内容随状态变化，位置自动避开选区。

import type { Rect } from './geometry';

export interface Hint {
  /** 按键或手势 */
  k: string;
  label: string;
}

/** 编辑态的阶段。标注、识别、长截图、录屏随对应任务加入。 */
export type HintState = 'idle' | 'selected' | 'annotate';

/**
 * 提示内容。只列出已经能用的操作，后续任务实现了新快捷键再补进来，
 * 不让提示里出现按了没反应的键。
 */
export function hintsFor(state: HintState, toolLabel = ''): Hint[] {
  switch (state) {
    case 'idle':
      return [
        { k: '拖动', label: '框选区域' },
        { k: '单击', label: '选中高亮的元素' },
        { k: '滚轮', label: '选择上级 / 下级元素' },
        { k: 'Enter', label: '复制高亮区域并结束' },
        { k: 'W A S D', label: '逐像素移动光标' },
        { k: 'P', label: '切换颜色格式' },
        { k: 'C', label: '复制颜色值' },
        { k: 'Esc', label: '退出' },
      ];
    case 'selected':
      return [
        { k: '拖动边框', label: '调整大小' },
        { k: '拖动内部', label: '移动选区' },
        { k: '方向键', label: '微调 1px（Shift 10px）' },
        { k: '空格', label: '开始标注' },
        { k: '1 – 7', label: '标注工具' },
        { k: 'Ctrl+C', label: '复制并结束' },
        { k: 'Ctrl+S', label: '保存…' },
        { k: 'Ctrl+T', label: '贴图并结束' },
        { k: 'P / C', label: '颜色格式 / 复制颜色' },
        { k: '右键', label: '取消选区' },
        { k: 'Esc', label: '退出' },
      ];
    case 'annotate':
      return [
        { k: '拖动', label: `绘制${toolLabel}` },
        { k: 'Shift', label: '正方形 / 正圆 / 45° 直线' },
        { k: '1 – 7', label: '切换标注工具' },
        { k: 'Ctrl+Z', label: '撤销' },
        { k: 'Ctrl+C', label: '复制并结束' },
        { k: '空格', label: '退出标注' },
        { k: 'Esc', label: '退出标注' },
      ];
  }
}

const overlaps = (a: Rect, b: Rect) =>
  a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height;

const overlapArea = (a: Rect, b: Rect) => {
  const w = Math.min(a.x + a.width, b.x + b.width) - Math.max(a.x, b.x);
  const h = Math.min(a.y + a.height, b.y + b.height) - Math.max(a.y, b.y);
  return w > 0 && h > 0 ? w * h : 0;
};

/**
 * 提示面板的位置（CSS 像素）。默认左下角；被选区挡住时依次试左上、右下、右上；
 * 四个角都被挡时选重叠面积最小的那个。
 */
export function placeHints(
  panel: { width: number; height: number },
  view: { width: number; height: number },
  avoid: Rect | null,
  margin = 16,
): { x: number; y: number } {
  const left = margin;
  const right = view.width - margin - panel.width;
  const top = margin;
  const bottom = view.height - margin - panel.height;
  const corners = [
    { x: left, y: bottom },
    { x: left, y: top },
    { x: right, y: bottom },
    { x: right, y: top },
  ];
  if (!avoid) return corners[0];
  const box = (c: { x: number; y: number }) => ({ ...c, width: panel.width, height: panel.height });
  const free = corners.find((c) => !overlaps(box(c), avoid));
  if (free) return free;
  return corners.reduce((best, c) => (overlapArea(box(c), avoid) < overlapArea(box(best), avoid) ? c : best));
}

/** 光标靠近面板（含外扩 pad 的范围）时，面板淡出，免得挡住要看的内容。 */
export function nearPanel(
  cursor: { x: number; y: number } | null,
  pos: { x: number; y: number },
  panel: { width: number; height: number },
  pad = 24,
): boolean {
  if (!cursor) return false;
  return (
    cursor.x >= pos.x - pad &&
    cursor.x <= pos.x + panel.width + pad &&
    cursor.y >= pos.y - pad &&
    cursor.y <= pos.y + panel.height + pad
  );
}
