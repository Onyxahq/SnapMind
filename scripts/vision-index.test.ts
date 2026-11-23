import { describe, expect, it } from 'vitest';
import { bareId, buildIndex } from './vision-index.mjs';

describe('vision-index', () => {
  it('normalizes ids like the Rust lookup does', () => {
    expect(bareId('openai/GPT-4o:free')).toBe('gpt-4o');
    expect(bareId('~x-ai/grok-latest')).toBe('grok-latest');
  });

  it("prefers the vendor's own entry, then the majority", () => {
    const m = (input: string[], reasoning = false) => ({ modalities: { input }, reasoning });
    const rows = buildIndex({
      relay1: { models: { 'acme/seer': m(['text']), 'acme/plain': m(['text', 'image']) } },
      relay2: { models: { 'acme/plain': m(['text']), 'acme/plain-2': m(['text']) } },
      relay3: { models: { 'acme/plain-2': m(['text']) } },
      openai: { models: { seer: m(['text', 'image'], true) } },
      relay4: { models: { 'acme/plain-2': m(['text', 'image']), 'no-modalities': {} } },
    });
    expect(rows).toEqual(['plain\tv', 'plain-2\t', 'seer\tvr']);
  });
});
