// 快捷键的录制与显示（设置 → 快捷键）。存储用 Tauri 的写法：修饰键 + 主键，用 + 连接，如 Ctrl+Shift+O。

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
  if (!key) return { kind: 'pending', hint: '这个键不能用作快捷键' };
  const standalone = /^F\d+$/.test(key) || key === 'PrintScreen' || key === 'Pause';
  if (!mods.length && !standalone) return { kind: 'pending', hint: '要配合 Ctrl、Alt、Shift 或 Win 键' };
  return { kind: 'keys', keys: [...mods, key].join('+') };
}

const SHOW: Record<string, string> = {
  Super: 'Win', Backquote: '`', Minus: '-', Equal: '=', BracketLeft: '[', BracketRight: ']', Backslash: '\\',
  Semicolon: ';', Quote: "'", Comma: ',', Period: '.', Slash: '/', ArrowUp: '↑', ArrowDown: '↓',
  ArrowLeft: '←', ArrowRight: '→', PageUp: 'PgUp', PageDown: 'PgDn', PrintScreen: 'PrtSc', Delete: 'Del',
  Insert: 'Ins', Space: '空格',
};

/** 显示用：Ctrl+Shift+O → Ctrl + Shift + O；空字符串 → 未设置。 */
export function displayKeys(keys: string): string {
  if (!keys.trim()) return '未设置';
  return keys
    .split('+')
    .map((p) => p.trim())
    .filter(Boolean)
    .map((p) => {
      const cap = p.length === 1 ? p.toUpperCase() : p;
      const norm = /^(ctrl|control)$/i.test(p) ? 'Ctrl' : /^(super|meta|win)$/i.test(p) ? 'Super' : cap;
      return SHOW[norm] ?? norm;
    })
    .join(' + ');
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
