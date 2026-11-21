import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MAX_TOASTS, dismiss, toast, toasts } from './toast.svelte';

describe('toast', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    toasts.length = 0;
  });
  afterEach(() => vi.useRealTimers());

  it('到时自动消失，错误提示停留更久', () => {
    toast('已复制');
    toast('失败', { kind: 'error' });
    expect(toasts.map((t) => t.text)).toEqual(['已复制', '失败']);
    vi.advanceTimersByTime(1600);
    expect(toasts.map((t) => t.text)).toEqual(['失败']);
    vi.advanceTimersByTime(1600);
    expect(toasts).toHaveLength(0);
  });

  it(`最多同时显示 ${MAX_TOASTS} 条，挤掉最早的`, () => {
    for (const s of ['a', 'b', 'c', 'd']) toast(s);
    expect(toasts.map((t) => t.text)).toEqual(['b', 'c', 'd']);
  });

  it('可以手动关闭', () => {
    const id = toast('x', { ms: 10_000 });
    dismiss(id);
    expect(toasts).toHaveLength(0);
  });
});
