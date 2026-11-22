import { describe, expect, it } from 'vitest';
import { displayKeys, keyName, record, sameKeys } from './hotkey';

const ev = (code: string, mods: Partial<Record<'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey', boolean>> = {}) => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe('hotkey', () => {
  it('names keys the way global-hotkey parses them', () => {
    expect(keyName('KeyO')).toBe('O');
    expect(keyName('Digit3')).toBe('3');
    expect(keyName('F12')).toBe('F12');
    expect(keyName('F25')).toBeNull();
    expect(keyName('Backquote')).toBe('Backquote');
    expect(keyName('IntlRo')).toBeNull();
  });

  it('records combos and special keys', () => {
    expect(record(ev('KeyO', { ctrlKey: true, shiftKey: true }))).toEqual({ kind: 'keys', keys: 'Ctrl+Shift+O' });
    expect(record(ev('F1', { shiftKey: true }))).toEqual({ kind: 'keys', keys: 'Shift+F1' });
    expect(record(ev('F2'))).toEqual({ kind: 'keys', keys: 'F2' });
    expect(record(ev('KeyD', { metaKey: true, altKey: true }))).toEqual({ kind: 'keys', keys: 'Alt+Super+D' });
    expect(record(ev('Escape'))).toEqual({ kind: 'cancel' });
    expect(record(ev('Backspace'))).toEqual({ kind: 'clear' });
    expect(record(ev('ShiftLeft', { shiftKey: true }))).toEqual({ kind: 'pending', hint: 'Shift + …' });
    expect(record(ev('KeyA')).kind).toBe('pending'); // 单个字母不行
  });

  it('displays and compares', () => {
    expect(displayKeys('Ctrl+Shift+O')).toBe('Ctrl + Shift + O');
    expect(displayKeys('Alt+Super+Backquote')).toBe('Alt + Win + `');
    expect(displayKeys('')).toBe('未设置');
    expect(sameKeys('Ctrl+Shift+S', 'shift+control+s')).toBe(true);
    expect(sameKeys('Ctrl+S', 'Ctrl+Shift+S')).toBe(false);
    expect(sameKeys('', '')).toBe(false);
  });
});
