import { describe, expect, it } from 'vitest';
import { appInfo, configGet, configUpdate, inTauri, mergePatch, onConfigChanged } from './ipc';

describe('ipc（浏览器假实现）', () => {
  it('返回占位应用信息', async () => {
    expect(inTauri()).toBe(false);
    const info = await appInfo();
    expect(info.name).toBe('SnapMind');
    expect(info.platform).toBe('browser');
  });

  it('配置更新会合并补丁并通知订阅者', async () => {
    const seen: string[] = [];
    const off = await onConfigChanged((c) => seen.push(c.appearance.theme));
    const next = await configUpdate({ appearance: { theme: 'dark' } });
    expect(next.appearance).toEqual({ theme: 'dark', accent: 'blue' });
    expect(next.capture.colorFormat).toBe('hex');
    expect((await configGet()).appearance.theme).toBe('dark');
    expect(seen).toEqual(['dark']);
    off();
    await configUpdate({ appearance: { theme: 'light' } });
    expect(seen).toEqual(['dark']);
  });
});

describe('mergePatch', () => {
  it('遵循 RFC 7386', () => {
    expect(mergePatch({ a: { b: 1, c: 2 }, d: [1] }, { a: { b: 9, c: null }, d: [3], e: 'x' })).toEqual({
      a: { b: 9 },
      d: [3],
      e: 'x',
    });
  });
  it('不修改原对象', () => {
    const t = { a: { b: 1 } };
    mergePatch(t, { a: { b: 2 } });
    expect(t.a.b).toBe(1);
  });
});
