import { describe, expect, it } from 'vitest';
import { canAdd, mergeModels, newProviderId, regionOf, toggleTag, visibleModels } from './providers';
import type { ModelConfig, ProviderConfig } from './ipc';

const p = (id: string, kind: ProviderConfig['kind']): ProviderConfig => ({ id, kind, name: id, baseUrl: '', models: [] });

describe('providers', () => {
  it('ids and adding rules', () => {
    expect(newProviderId('deepseek', [])).toBe('deepseek');
    expect(newProviderId('custom', [p('custom', 'custom'), p('custom-2', 'custom')])).toBe('custom-3');
    expect(canAdd('deepseek', [p('deepseek', 'deepseek')])).toBe(false);
    expect(canAdd('custom', [p('custom', 'custom')])).toBe(true);
  });

  it('merge keeps user choices and manual models', () => {
    const cur: ModelConfig[] = [
      { id: 'a', tags: ['vision'], enabled: true },
      { id: 'manual', tags: ['text'], enabled: true },
      { id: 'gone', tags: ['text'], enabled: false },
    ];
    const out = mergeModels(cur, [
      { id: 'a', tags: ['text'] },
      { id: 'b', tags: ['text', 'reasoning'] },
      { id: 'b', tags: ['text'] },
    ]);
    expect(out).toEqual([
      { id: 'manual', tags: ['text'], enabled: true },
      { id: 'a', tags: ['vision'], enabled: true },
      { id: 'b', tags: ['text', 'reasoning'], enabled: false },
    ]);
  });

  it('visible models: enabled first, filtered', () => {
    const ms: ModelConfig[] = [
      { id: 'x-1', tags: [], enabled: false },
      { id: 'y-2', tags: [], enabled: true },
      { id: 'x-3', tags: [], enabled: true },
    ];
    expect(visibleModels(ms).map((m) => m.id)).toEqual(['y-2', 'x-3', 'x-1']);
    expect(visibleModels(ms, ' X ').map((m) => m.id)).toEqual(['x-3', 'x-1']);
  });

  it('tag toggling keeps vision/text exclusive and non-empty', () => {
    expect(toggleTag(['text'], 'vision')).toEqual(['vision']);
    expect(toggleTag(['vision'], 'vision')).toEqual(['text']);
    expect(toggleTag(['text'], 'reasoning')).toEqual(['text', 'reasoning']);
    expect(toggleTag(['text', 'reasoning'], 'reasoning')).toEqual(['text']);
    expect(toggleTag(['reasoning', 'text'], 'local')).toEqual(['text', 'reasoning', 'local']);
  });
});

describe('regionOf', () => {
  it('tells mainland endpoints from international ones', () => {
    expect(regionOf('https://api.moonshot.cn/v1')).toBe('国内站');
    expect(regionOf('https://open.bigmodel.cn/api/paas/v4')).toBe('国内站');
    expect(regionOf('https://dashscope.aliyuncs.com/compatible-mode/v1')).toBe('国内站');
    expect(regionOf('https://api.minimaxi.com/v1')).toBe('国内站');
    expect(regionOf('https://api.moonshot.ai/v1')).toBe('国际站');
    expect(regionOf('https://dashscope-intl.aliyuncs.com/compatible-mode/v1')).toBe('国际站');
    expect(regionOf('https://api.minimax.io/v1')).toBe('国际站');
    expect(regionOf('https://api.siliconflow.com/v1')).toBe('国际站');
  });
});
