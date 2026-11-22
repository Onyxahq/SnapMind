// 识别 / 翻译任务在卡片上的状态：把 Rust 推来的事件折叠成卡片要显示的内容，以及卡片摆放位置。
import type { Rect } from './geometry';
import type { TaskEvent } from './ipc';

export type CardStage = 'recognize' | 'translate' | 'done' | 'error';

export interface TranslateCard {
  id: string;
  src: string;
  dst: string;
  stage: CardStage;
  /** 正在用的后端 */
  backend: string;
  /** 完成后的说明：实际使用的后端、切换原因、耗时 */
  meta: string;
  error: string;
}

export function newCard(id: string, src = '', stage: CardStage = 'recognize'): TranslateCard {
  return { id, src, dst: '', stage, backend: '', meta: '', error: '' };
}

/** 应用一个事件；不是这张卡片的事件原样返回。 */
export function reduce(c: TranslateCard, e: TaskEvent): TranslateCard {
  if (e.id !== c.id) return c;
  switch (e.kind) {
    case 'stage':
      return { ...c, stage: e.stage, backend: e.text, error: '' };
    case 'src':
      return { ...c, src: c.src + e.text };
    case 'src-reset':
      return { ...c, src: '' };
    case 'dst':
      return { ...c, dst: c.dst + e.text };
    case 'dst-reset':
      return { ...c, dst: '' };
    case 'done':
      return { ...c, stage: 'done', meta: e.text, src: c.src.trim(), dst: c.dst.trim() };
    case 'error':
      return { ...c, stage: 'error', error: e.text };
  }
}

/** 卡片标题上的状态文字。 */
export function stageLabel(c: TranslateCard): string {
  const who = c.backend ? ` · ${c.backend}` : '';
  switch (c.stage) {
    case 'recognize':
      return `识别中${who}`;
    case 'translate':
      return `翻译中${who}`;
    case 'done':
      return '翻译';
    case 'error':
      return '出错了';
  }
}

/**
 * 结果卡片的位置（CSS 像素）：优先放在选区右侧，其次左侧，再其次下方、上方，
 * 都放不下就贴着屏幕右边；纵向和选区顶部对齐并夹在屏幕内。
 */
export function placeCard(
  sel: Rect,
  card: { width: number; height: number },
  view: { width: number; height: number },
  gap = 12,
  margin = 8,
): { x: number; y: number } {
  const clampY = (y: number) => Math.min(Math.max(y, margin), Math.max(margin, view.height - card.height - margin));
  const clampX = (x: number) => Math.min(Math.max(x, margin), Math.max(margin, view.width - card.width - margin));
  const right = sel.x + sel.width + gap;
  if (right + card.width <= view.width - margin) return { x: right, y: clampY(sel.y) };
  const left = sel.x - gap - card.width;
  if (left >= margin) return { x: left, y: clampY(sel.y) };
  const below = sel.y + sel.height + gap;
  if (below + card.height <= view.height - margin) return { x: clampX(sel.x), y: below };
  const above = sel.y - gap - card.height;
  if (above >= margin) return { x: clampX(sel.x), y: above };
  return { x: view.width - card.width - margin, y: clampY(sel.y) };
}
