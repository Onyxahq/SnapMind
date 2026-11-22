import { describe, expect, it } from 'vitest';
import { bilingual, newCard, parseGlossary, placeCard, reduce, stageLabel } from './task';
import { unionRect } from './geometry';
import type { TaskEvent } from './ipc';

const ev = (kind: TaskEvent['kind'], text = '', stage: TaskEvent['stage'] = 'translate'): TaskEvent => ({ id: 'a', kind, stage, text });

describe('task card', () => {
  it('folds a full recognize → translate run with a switch', () => {
    let c = newCard('a');
    for (const e of [
      ev('stage', '', 'recognize'),
      ev('stage', 'qwen-vl', 'recognize'),
      ev('src', 'Hel', 'recognize'),
      ev('src', 'lo\n', 'recognize'),
      ev('stage', 'deepseek-chat'),
      ev('dst', '半截'),
      ev('dst-reset'),
      ev('stage', 'DeepL'),
      ev('dst', '你好'),
      ev('done', 'qwen-vl → deepseek-chat 超时，已切换到 DeepL · 1.2 s'),
    ])
      c = reduce(c, e);
    expect(c).toMatchObject({ src: 'Hello', dst: '你好', stage: 'done', backend: 'DeepL' });
    expect(stageLabel(c)).toBe('翻译');
    expect(reduce(c, { ...ev('dst', 'x'), id: 'other' })).toBe(c);
  });

  it('bilingual text and glossary parsing', () => {
    expect(bilingual('A\n\nB', '甲\n\n乙')).toBe('A\n甲\n\nB\n乙');
    expect(bilingual('A\n\nB', '甲乙')).toBe('A\n\nB\n\n甲乙');
    expect(bilingual('', '甲')).toBe('甲');
    expect(parseGlossary(' Rust, Tauri、Rust ；\nSvelte ')).toEqual(['Rust', 'Tauri', 'Svelte']);
    expect(stageLabel(reduce(newCard('a'), ev('stage', 'claude', 'oneshot')))).toBe('一步到位 · claude');
  });

  it('errors and labels', () => {
    const c = reduce(reduce(newCard('a'), ev('stage', 'm', 'recognize')), ev('error', '识别失败'));
    expect(c.stage).toBe('error');
    expect(c.error).toBe('识别失败');
    expect(stageLabel(reduce(newCard('a'), ev('stage', 'x', 'recognize')))).toBe('识别中 · x');
  });

  it('places the card beside, then below, then inside the screen', () => {
    const card = { width: 380, height: 300 };
    const view = { width: 1920, height: 1080 };
    expect(placeCard({ x: 100, y: 100, width: 400, height: 200 }, card, view)).toEqual({ x: 512, y: 100 });
    expect(placeCard({ x: 1400, y: 100, width: 500, height: 200 }, card, view)).toEqual({ x: 1008, y: 100 });
    expect(placeCard({ x: 10, y: 100, width: 1900, height: 200 }, card, view)).toEqual({ x: 10, y: 312 });
    expect(placeCard({ x: 0, y: 0, width: 1920, height: 1080 }, card, view)).toEqual({ x: 1532, y: 8 });
    // 选区靠下：纵向夹在屏幕内
    expect(placeCard({ x: 100, y: 1000, width: 200, height: 50 }, card, view).y).toBe(772);
    // 避开比选区宽的工具条
    const avoid = unionRect({ x: 100, y: 100, width: 300, height: 200 }, { x: 60, y: 308, width: 420, height: 38 });
    expect(placeCard(avoid, card, view).x).toBe(492);
    expect(unionRect(avoid, null)).toBe(avoid);
  });
});
