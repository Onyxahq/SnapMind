import { msg } from './i18n.svelte';
// 明暗模式与强调色。把结果写到 <html data-theme / data-accent>，tokens.css 据此切换变量。

export type ThemeMode = 'system' | 'light' | 'dark';
export type Accent = 'blue' | 'sky' | 'violet' | 'green';
export type Resolved = 'light' | 'dark';

export const ACCENTS: { id: Accent; label: string; light: string; dark: string }[] = [
  { id: 'blue', label: msg('蓝'), light: '#2563EB', dark: '#60A5FA' },
  { id: 'sky', label: msg('天蓝'), light: '#0284C7', dark: '#38BDF8' },
  { id: 'violet', label: msg('紫'), light: '#7C3AED', dark: '#A78BFA' },
  { id: 'green', label: msg('绿'), light: '#059669', dark: '#34D399' },
];

/** 把「跟随系统」解析成具体的明暗。 */
export function resolveTheme(mode: ThemeMode, systemDark: boolean): Resolved {
  if (mode === 'system') return systemDark ? 'dark' : 'light';
  return mode;
}

const darkQuery = () =>
  typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null;

let current: { mode: ThemeMode; accent: Accent } = { mode: 'system', accent: 'blue' };
let listening = false;

function paint(animate: boolean) {
  const root = document.documentElement;
  const resolved = resolveTheme(current.mode, darkQuery()?.matches ?? false);
  if (animate) root.classList.add('theme-anim');
  root.dataset.theme = resolved;
  root.dataset.accent = current.accent;
}

const reduceQuery = () =>
  typeof matchMedia === 'function' ? matchMedia('(prefers-reduced-motion: reduce)') : null;
let motionPref: 'full' | 'reduced' = 'full';
let listeningMotion = false;

function paintMotion() {
  const reduce = motionPref === 'reduced' || (reduceQuery()?.matches ?? false);
  document.documentElement.dataset.motion = reduce ? 'reduced' : 'full';
}

/** 动效：设置里选了「减弱」，或者系统关掉了动画，都只保留淡入淡出。 */
export function applyMotion(pref: 'full' | 'reduced'): void {
  motionPref = pref;
  paintMotion();
  if (!listeningMotion) {
    reduceQuery()?.addEventListener('change', paintMotion);
    listeningMotion = true;
  }
}

/**
 * 应用主题。第一次调用不做过渡（避免窗口打开时闪一下），之后的切换有 300ms 颜色过渡。
 * 「跟随系统」时会监听系统明暗变化。
 */
export function applyTheme(mode: ThemeMode, accent: Accent): void {
  const first = !document.documentElement.dataset.theme;
  current = { mode, accent };
  paint(!first);
  if (!listening) {
    darkQuery()?.addEventListener('change', () => {
      if (current.mode === 'system') paint(true);
    });
    listening = true;
  }
}
