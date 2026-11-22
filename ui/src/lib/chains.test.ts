import { describe, expect, it } from 'vitest';
import { available, move, normalize, pool, refKey, refLabel, refState } from './chains';
import type { BackendRef, ProviderConfig } from './ipc';

const providers: ProviderConfig[] = [
  {
    id: 'deepseek',
    kind: 'deepseek',
    name: 'DeepSeek',
    baseUrl: '',
    models: [
      { id: 'deepseek-chat', tags: ['text'], enabled: true },
      { id: 'deepseek-old', tags: ['text'], enabled: false },
    ],
  },
  { id: 'ollama', kind: 'ollama', name: 'Ollama', baseUrl: '', models: [{ id: 'qwen-vl', tags: ['vision', 'local'], enabled: true }] },
];
const keys = { deepseek: true, 'mt-deepl': true };
const ds: BackendRef = { type: 'llm', provider: 'deepseek', model: 'deepseek-chat' };

describe('chains', () => {
  it('pools per task', () => {
    expect(pool('translate', providers, keys).map(refKey)).toEqual(['llm:deepseek/deepseek-chat', 'llm:ollama/qwen-vl', 'mt:deepl']);
    expect(pool('recognize', providers, keys).map(refKey)).toEqual(['localOcr', 'llm:ollama/qwen-vl']);
    expect(pool('oneshot', providers, keys).map(refKey)).toEqual(['llm:ollama/qwen-vl', 'degrade']);
    expect(available('translate', [ds], providers, keys).map(refKey)).toEqual(['llm:ollama/qwen-vl', 'mt:deepl']);
  });

  it('labels and states', () => {
    expect(refLabel(ds, providers)).toBe('DeepSeek · deepseek-chat');
    expect(refState(ds, 'translate', providers, keys, ['ollama'])).toBe('ok');
    expect(refState(ds, 'recognize', providers, keys, ['ollama'])).toBe('stale');
    expect(refState({ type: 'llm', provider: 'deepseek', model: 'deepseek-old' }, 'translate', providers, keys, ['ollama'])).toBe('stale');
    expect(refState({ type: 'llm', provider: 'gone', model: 'x' }, 'translate', providers, keys, ['ollama'])).toBe('stale');
    expect(refState({ type: 'llm', provider: 'ollama', model: 'qwen-vl' }, 'oneshot', providers, keys, ['ollama'])).toBe('ok');
    expect(refState(ds, 'translate', providers, {}, ['ollama'])).toBe('nokey');
    expect(refState({ type: 'mt', kind: 'google' }, 'translate', providers, keys, [])).toBe('nokey');
  });

  it('degrade stays last; move bounds', () => {
    const vl: BackendRef = { type: 'llm', provider: 'ollama', model: 'qwen-vl' };
    expect(normalize('oneshot', [{ type: 'degrade' }, vl])).toEqual([vl, { type: 'degrade' }]);
    expect(normalize('translate', [ds])).toEqual([ds]);
    expect(move([1, 2, 3], 0, 2)).toEqual([2, 3, 1]);
    expect(move([1, 2, 3], 0, -1)).toEqual([1, 2, 3]);
  });
});
