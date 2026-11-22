// 识别 / 翻译任务在卡片上的状态：把 Rust 推来的事件折叠成卡片要显示的内容，以及卡片摆放位置。
import type { Rect } from './geometry';
import type { TaskEvent } from './ipc';
import type { OverlayBlock } from './overlay';

export type CardStage = 'recognize' | 'oneshot' | 'translate' | 'markdown' | 'formula' | 'done' | 'error';

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
  /** 原位覆盖的段落块（覆盖模式翻译完成后才有） */
  overlay: OverlayBlock[] | null;
}

export function newCard(id: string, src = '', stage: CardStage = 'recognize'): TranslateCard {
  return { id, src, dst: '', stage, backend: '', meta: '', error: '', overlay: null };
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
    case 'overlay':
      try {
        return { ...c, overlay: JSON.parse(e.text) as OverlayBlock[] };
      } catch {
        return c;
      }
  }
}

/** 卡片标题上的状态文字。 */
export function stageLabel(c: TranslateCard): string {
  const who = c.backend ? ` · ${c.backend}` : '';
  switch (c.stage) {
    case 'recognize':
      return `识别中${who}`;
    case 'oneshot':
      return `一步到位${who}`;
    case 'translate':
      return `翻译中${who}`;
    case 'markdown':
    case 'formula':
      return `识别中${who}`;
    case 'done':
      return '翻译';
    case 'error':
      return '出错了';
  }
}

/**
 * 双语对照：原文和译文按段落（空行分隔）交替排列；段落数对不上时，原文在前、译文在后。
 */
export function bilingual(src: string, dst: string): string {
  const split = (s: string) => s.trim().split(/\n\s*\n/).map((p) => p.trim()).filter(Boolean);
  const a = split(src);
  const b = split(dst);
  if (!a.length) return dst.trim();
  if (a.length !== b.length) return `${src.trim()}\n\n${dst.trim()}`;
  return a.map((p, i) => `${p}\n${b[i]}`).join('\n\n');
}

/** 不翻译列表：逗号、顿号、分号或换行分隔，去掉空白和重复。 */
export function parseGlossary(text: string): string[] {
  return [...new Set(text.split(/[,，、;；\n]/).map((s) => s.trim()).filter(Boolean))];
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
