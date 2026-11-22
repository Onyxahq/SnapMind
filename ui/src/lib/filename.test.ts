import { describe, expect, it } from 'vitest';
import { renderFilename } from './filename';

describe('renderFilename', () => {
  const t = new Date(2026, 10, 22, 9, 5, 7, 42);
  it('replaces time tokens inside braces only', () => {
    expect(renderFilename('SnapMind_{yyyyMMdd_HHmmss}', t)).toBe('SnapMind_20261122_090507');
    expect(renderFilename('{yyyy-MM-dd} HH {SSS}', t)).toBe('2026-11-22 HH 042');
  });
  it('keeps unclosed braces and sanitizes', () => {
    expect(renderFilename('a{yyyy', t)).toBe('a{yyyy');
    expect(renderFilename('a/b:c?{dd}. ', t)).toBe('a_b_c_22');
    expect(renderFilename('  ', t)).toBe('SnapMind');
  });
});
