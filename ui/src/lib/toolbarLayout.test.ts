import { describe, expect, it } from 'vitest';
import { CAPTURE_ITEMS, PIN_ITEMS, layoutRows, toLayout, visibleBar, MORE } from './toolbar';
import { DEFAULT_CONFIG } from './ipc';

const ids = (xs: { id: string }[]) => xs.map((x) => x.id);

describe('toolbar layout', () => {
  it('default layouts match the current toolbars', () => {
    const cap = visibleBar(DEFAULT_CONFIG.toolbar.capture, CAPTURE_ITEMS);
    expect(ids(cap.more)).toEqual(['formula', 'table']);
    expect(ids(cap.main)).toEqual(['rect', 'pen', 'arrow', 'text', 'number', 'mosaic', 'eraser', 'undo', 'long', 'record', 'ocr', 'markdown', 'translate', 'pin', 'save']);
    const pin = visibleBar(DEFAULT_CONFIG.toolbar.pin, PIN_ITEMS);
    expect(ids(pin.main)).toEqual(['annotate', 'save', 'copy']);
    expect(ids(pin.more)).toEqual(['through', 'zoom100']);
  });

  it('hides, moves into more, ignores unknown ids and duplicates', () => {
    const v = visibleBar({ order: ['save', 'nope', 'copy', 'save', MORE, 'annotate'], hidden: ['copy'] }, PIN_ITEMS);
    // through、zoom100 不在布局里：插到分隔线前面
    expect(ids(v.main)).toEqual(['save', 'through', 'zoom100']);
    expect(ids(v.more)).toEqual(['annotate']);
  });

  it('adds a divider when missing and round-trips', () => {
    const rows = layoutRows({ order: ['copy', 'save'], hidden: [] }, PIN_ITEMS);
    expect(rows.filter((r) => 'divider' in r)).toHaveLength(1);
    expect('divider' in rows[rows.length - 1]).toBe(true);
    const back = toLayout(rows);
    expect(back.order.slice(0, 2)).toEqual(['copy', 'save']);
    expect(back.order.at(-1)).toBe(MORE);
    expect(toLayout(layoutRows(back, PIN_ITEMS))).toEqual(back);
  });
});
