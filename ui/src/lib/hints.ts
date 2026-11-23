// 截图编辑态的操作提示：内容随状态变化，位置自动避开选区。

import type { Rect } from './geometry';
import { t } from './i18n.svelte';

export interface Hint {
  /** 按键或手势 */
  k: string;
  label: string;
}

/** 编辑态的阶段。标注、识别、长截图、录屏随对应任务加入。 */
export type HintState = 'idle' | 'selected' | 'annotate' | 'ocr' | 'long' | 'rec-setup' | 'rec';

/**
 * 提示内容。只列出已经能用的操作，后续任务实现了新快捷键再补进来，
 * 不让提示里出现按了没反应的键。
 */
export function hintsFor(state: HintState, toolLabel = ''): Hint[] {
  switch (state) {
    case 'idle':
      return [
        { k: t('拖动'), label: t('框选区域') },
        { k: t('单击'), label: t('选中高亮的元素') },
        { k: t('滚轮'), label: t('选择上级 / 下级元素') },
        { k: 'Enter', label: t('复制高亮区域并结束') },
        { k: 'W A S D', label: t('逐像素移动光标') },
        { k: 'P', label: t('切换颜色格式') },
        { k: 'C', label: t('复制颜色值') },
        { k: 'Esc', label: t('退出') },
      ];
    case 'selected':
      return [
        { k: t('拖动边框'), label: t('调整大小') },
        { k: t('拖动内部'), label: t('移动选区') },
        { k: t('方向键'), label: t('微调 1px（Shift 10px）') },
        { k: t('空格'), label: t('开始标注') },
        { k: '1 – 7', label: t('标注工具') },
        { k: 'Ctrl+C', label: t('复制并结束') },
        { k: 'Ctrl+S', label: t('保存…') },
        { k: 'Ctrl+T', label: t('贴图并结束') },
        { k: 'Shift+C', label: t('识别文字') },
        { k: 'Shift+Alt+R', label: t('识别为 Markdown') },
        { k: 'Ctrl+Q', label: t('翻译') },
        { k: 'L / R', label: t('长截图 / 录屏') },
        { k: 'P / C', label: t('颜色格式 / 复制颜色') },
        { k: t('右键'), label: t('取消选区') },
        { k: 'Esc', label: t('退出') },
      ];
    case 'annotate':
      return [
        { k: t('拖动'), label: t('绘制{toolLabel}', { toolLabel }) },
        { k: 'Shift', label: t('正方形 / 正圆 / 45° 直线') },
        { k: '1 – 7', label: t('切换标注工具') },
        { k: 'Ctrl+Z', label: t('撤销') },
        { k: 'Ctrl+C', label: t('复制并结束') },
        { k: t('空格'), label: t('退出标注') },
        { k: 'Esc', label: t('退出标注') },
      ];
    case 'ocr':
      return [
        { k: t('单击框'), label: t('选择 / 取消这一行') },
        { k: 'Ctrl+A', label: t('全选 / 全不选') },
        { k: 'Ctrl+C', label: t('复制文字并结束') },
        { k: 'Esc', label: t('关闭识别卡片') },
      ];
    case 'long':
      return [
        { k: t('滚轮'), label: t('滚动页面，自动拼接') },
        { k: 'Enter', label: t('完成并复制') },
        { k: 'Ctrl+T', label: t('贴图') },
        { k: 'Ctrl+S', label: t('保存') },
        { k: 'Shift+C', label: t('识别文字') },
        { k: 'Esc', label: t('取消长截图') },
      ];
    case 'rec-setup':
      return [
        { k: 'R', label: t('开始录制') },
        { k: 'Esc', label: t('返回') },
      ];
    case 'rec':
      return [
        { k: 'R', label: t('停止并保存') },
        { k: t('空格'), label: t('暂停 / 继续') },
        { k: 'Esc', label: t('放弃录屏') },
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
