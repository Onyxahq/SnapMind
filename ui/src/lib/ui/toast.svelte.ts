// 轻提示队列。任何组件调用 toast('已复制 96 字') 即可，页面里放一个 <ToastHost /> 负责显示。

export type ToastKind = 'ok' | 'info' | 'error';
export interface ToastItem {
  id: number;
  text: string;
  kind: ToastKind;
}

/** 同时最多显示几条；更早的直接移除。 */
export const MAX_TOASTS = 3;

export const toasts = $state<ToastItem[]>([]);
let seq = 0;

export function toast(text: string, opts: { kind?: ToastKind; ms?: number } = {}): number {
  const item: ToastItem = { id: ++seq, text, kind: opts.kind ?? 'ok' };
  toasts.push(item);
  while (toasts.length > MAX_TOASTS) toasts.shift();
  const ms = opts.ms ?? (item.kind === 'error' ? 3200 : 1600);
  setTimeout(() => dismiss(item.id), ms);
  return item.id;
}

export function dismiss(id: number): void {
  const i = toasts.findIndex((t) => t.id === id);
  if (i >= 0) toasts.splice(i, 1);
}
