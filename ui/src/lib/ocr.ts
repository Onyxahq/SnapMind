// 识别文字卡片（需求 3.8、M4.4）的纯逻辑：段落框、点框挑行、状态文字。
// 坐标都是相对选区左上角的截图物理像素（和 Rust 返回的行框一致）。
import type { Rect } from './geometry';
import type { OcrResult, OcrStatus } from './ipc';
import { t } from './i18n.svelte';

export interface OcrCard {
  id: string;
  /** 识别时的选区（截图物理像素）；选区变了检测框就不再画 */
  region: Rect;
  /** run 识别中；model 需要先下载模型；done 完成；error 出错 */
  stage: 'run' | 'model' | 'done' | 'error';
  result: OcrResult | null;
  /** 卡片里的文字（可编辑） */
  text: string;
  byLine: boolean;
  /** 选中的行号（升序）；识别完默认全选 */
  pick: number[];
  error: string;
}

export function newOcrCard(id: string, region: Rect): OcrCard {
  return { id, region, stage: 'run', result: null, text: '', byLine: false, pick: [], error: '' };
}

/** 一组矩形的外接矩形。 */
export function bounds(rects: Rect[]): Rect {
  const x0 = Math.min(...rects.map((r) => r.x));
  const y0 = Math.min(...rects.map((r) => r.y));
  const x1 = Math.max(...rects.map((r) => r.x + r.width));
  const y1 = Math.max(...rects.map((r) => r.y + r.height));
  return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
}

/** 每个段落的框（只算 keep 里的行；段落里一行都没选中就不画）。 */
export function paragraphBoxes(r: OcrResult, keep?: Set<number>): Rect[] {
  return r.paragraphs
    .map((p) => p.filter((i) => !keep || keep.has(i)))
    .filter((p) => p.length)
    .map((p) => bounds(p.map((i) => r.lines[i])));
}

/** 点（相对识别区域的截图像素）落在哪一行的检测框里；框比文字外扩 pad。没有返回 -1。 */
export function hitLine(r: OcrResult, p: { x: number; y: number }, pad = 0): number {
  return r.lines.findIndex((l) => p.x >= l.x - pad && p.x <= l.x + l.width + pad && p.y >= l.y - pad && p.y <= l.y + l.height + pad);
}

/** 左键点检测框：已选的变未选，未选的变已选。 */
export function toggleLine(pick: number[], i: number): number[] {
  return pick.includes(i) ? pick.filter((x) => x !== i) : [...pick, i].sort((a, b) => a - b);
}

/** 全部行号。 */
export function allLines(r: OcrResult): number[] {
  return r.lines.map((_, i) => i);
}

/** Ctrl+A：已经全选就全不选，否则全选。 */
export function toggleAll(r: OcrResult, pick: number[]): number[] {
  return pick.length === r.lines.length ? [] : allLines(r);
}

/** 下载进度：「8.3 / 20.5 MB」。 */
export function progressLabel(s: OcrStatus): string {
  const mb = (n: number) => (n / 1024 / 1024).toFixed(1);
  const from = s.source ? ` · ${s.source}` : '';
  return `${mb(s.received)} / ${mb(s.total)} MB${from}`;
}

/** 卡片底部的说明：「12 行 · 4 段 · 0.21 s」，没全选时「已选 3 / 12 行」。 */
export function ocrMeta(c: OcrCard): string {
  const r = c.result;
  if (!r) return '';
  if (c.pick.length < r.lines.length) return t('已选 {length} / {length2} 行', { length: c.pick.length, length2: r.lines.length });
  return t('{length} 行 · {length2} 段 · 本地 OCR {toFixed} s', { length: r.lines.length, length2: r.paragraphs.length, toFixed: (r.ms / 1000).toFixed(2) });
}
