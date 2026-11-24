// 快捷键的录制与显示（设置 → 快捷键）。存储用 Tauri 的写法：修饰键 + 主键，用 + 连接，如 Ctrl+Shift+O。
import type { HotkeyAction } from './ipc';
import { t, msg } from './i18n.svelte';

/** 全局快捷键（设置 → 快捷键、新手引导共用）。 */
export const GLOBAL_HOTKEYS: { action: HotkeyAction; name: string; desc: string }[] = [
  { action: 'capture', name: msg('截图'), desc: msg('进入截图编辑态') },
  { action: 'pinClipboard', name: msg('贴图'), desc: msg('把剪贴板里的图片或文字贴到屏幕上') },
  { action: 'selection', name: msg('划词翻译'), desc: msg('翻译当前选中的文字') },
  { action: 'captureTranslate', name: msg('截图 OCR 后翻译'), desc: msg('框选后直接出翻译卡片') },
  { action: 'captureRecognize', name: msg('直接截图 OCR'), desc: msg('框选后复制识别出的文字') },
];

/** 是不是 macOS：快捷键显示成 ⌘ ⌥ ⇧ ⌃，录制提示也换成 Mac 的键名。 */
export const IS_MAC = typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform || navigator.userAgent);

const MODS: ['ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey', string][] = [
  ['ctrlKey', 'Ctrl'],
  ['altKey', 'Alt'],
  ['shiftKey', 'Shift'],
  ['metaKey', 'Super'],
];

/** 单独按下时不构成快捷键的键（修饰键本身）。 */
const MODIFIER_CODES = /^(Control|Shift|Alt|Meta|OS)(Left|Right)?$/;

/** 按 KeyboardEvent.code 得到主键名（Tauri / global-hotkey 认得的写法）。不支持的键返回 null。 */
export function keyName(code: string): string | null {
  let m: RegExpMatchArray | null;
  if ((m = code.match(/^Key([A-Z])$/))) return m[1];
  if ((m = code.match(/^Digit(\d)$/))) return m[1];
  if (/^F([1-9]|1\d|2[0-4])$/.test(code)) return code;
  if ((m = code.match(/^Numpad(\d)$/))) return `Num${m[1]}`;
  const named = [
    'Space', 'Enter', 'Tab', 'Backquote', 'Minus', 'Equal', 'BracketLeft', 'BracketRight', 'Backslash',
    'Semicolon', 'Quote', 'Comma', 'Period', 'Slash', 'Insert', 'Delete', 'Home', 'End', 'PageUp', 'PageDown',
    'ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'PrintScreen', 'Pause', 'ScrollLock',
  ];
  return named.includes(code) ? code : null;
}

export type Recorded =
  | { kind: 'keys'; keys: string }
  /** Esc：取消录制 */
  | { kind: 'cancel' }
  /** Backspace / Delete（不带修饰键）：清除，不用这个快捷键 */
  | { kind: 'clear' }
  /** 还没按完（只按了修饰键），或这个组合不能用 */
  | { kind: 'pending'; hint: string };

/** 录制时的一次按键。主键必须配修饰键（F1–F24、PrintScreen、Pause 除外，免得占用普通打字）。 */
export function record(e: Pick<KeyboardEvent, 'code' | 'ctrlKey' | 'altKey' | 'shiftKey' | 'metaKey'>): Recorded {
  const mods = MODS.filter(([k]) => e[k]).map(([, n]) => n);
  if (MODIFIER_CODES.test(e.code)) return { kind: 'pending', hint: mods.length ? `${mods.join(' + ')} + …` : '' };
  if (!mods.length && e.code === 'Escape') return { kind: 'cancel' };
  if (!mods.length && (e.code === 'Backspace' || e.code === 'Delete')) return { kind: 'clear' };
  const key = keyName(e.code);
  if (!key) return { kind: 'pending', hint: t('这个键不能用作快捷键') };
  const standalone = /^F\d+$/.test(key) || key === 'PrintScreen' || key === 'Pause';
  if (!mods.length && !standalone) {
    return { kind: 'pending', hint: IS_MAC ? t('要配合 ⌘、⌥、⌃ 或 ⇧ 键') : t('要配合 Ctrl、Alt、Shift 或 Win 键') };
  }
  return { kind: 'keys', keys: [...mods, key].join('+') };
}

const SHOW: Record<string, string> = {
  Super: 'Win', Backquote: '`', Minus: '-', Equal: '=', BracketLeft: '[', BracketRight: ']', Backslash: '\\',
  Semicolon: ';', Quote: "'", Comma: ',', Period: '.', Slash: '/', ArrowUp: '↑', ArrowDown: '↓',
  ArrowLeft: '←', ArrowRight: '→', PageUp: 'PgUp', PageDown: 'PgDn', PrintScreen: 'PrtSc', Delete: 'Del',
  Insert: 'Ins',
};

/** Mac 菜单里修饰键的固定顺序：⌃ ⌥ ⇧ ⌘。 */
const MAC_MODS: [string, string][] = [['Ctrl', '⌃'], ['Alt', '⌥'], ['Shift', '⇧'], ['Super', '⌘']];

/**
 * 显示全局快捷键（Tauri 写法）：Ctrl+Shift+O → Ctrl + Shift + O；空字符串 → 未设置。
 * Mac 上按系统习惯写成符号：Super+Shift+1 → ⇧⌘1，Ctrl 就是 Control 键 → ⌃。
 */
export function displayKeys(keys: string, mac = IS_MAC): string {
  if (!keys.trim()) return t('未设置');
  const parts = keys
    .split('+')
    .map((p) => p.trim())
    .filter(Boolean)
    .map((p) => {
      const cap = p.length === 1 ? p.toUpperCase() : p;
      return /^(ctrl|control)$/i.test(p) ? 'Ctrl' : /^(super|meta|win|cmd|command)$/i.test(p) ? 'Super' : /^(alt|option)$/i.test(p) ? 'Alt' : /^shift$/i.test(p) ? 'Shift' : cap;
    });
  const show = (p: string) => (p === 'Space' ? t('空格') : (SHOW[p] ?? p));
  if (!mac) return parts.map(show).join(' + ');
  const mods = MAC_MODS.filter(([n]) => parts.includes(n)).map(([, s]) => s).join('');
  return mods + parts.filter((p) => !MAC_MODS.some(([n]) => n === p)).map(show).join('');
}

/**
 * 显示程序内的快捷键（工具条、提示、设置页里写死的 Ctrl+C 一类）。这些键在 Mac 上按 ⌘ 也行
 * （处理时判断 ctrlKey || metaKey），所以 Mac 上 Ctrl 显示成 ⌘：Ctrl+C → ⌘C，Shift+Alt+R → ⌥⇧R。
 * 可以传整句文字，只替换其中的组合键。
 */
export function appKeys(text: string, mac = IS_MAC): string {
  if (!mac) return text;
  const sym: Record<string, string> = { Alt: '⌥', Shift: '⇧', Ctrl: '⌘' };
  return text.replace(/\b(?:(?:Ctrl|Alt|Shift)\+)+/g, (m) =>
    ['Alt', 'Shift', 'Ctrl'].filter((k) => m.includes(`${k}+`)).map((k) => sym[k]).join(''),
  );
}

/** 比较两个快捷键是否相同（不区分大小写和修饰键顺序）。 */
export function sameKeys(a: string, b: string): boolean {
  const norm = (s: string) =>
    s
      .split('+')
      .map((p) => p.trim().toLowerCase().replace(/^control$/, 'ctrl').replace(/^(meta|win)$/, 'super'))
      .filter(Boolean)
      .sort()
      .join('+');
  return !!a.trim() && norm(a) === norm(b);
}
