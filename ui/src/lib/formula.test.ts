import { describe, expect, it } from 'vitest';
import { renderLatex, splitFormulas, toDisplayMath } from './formula';

describe('formula', () => {
  it('renders LaTeX and reports errors instead of throwing', () => {
    const ok = renderLatex('\\frac{a}{b}');
    expect(ok.error).toBe('');
    expect(ok.html).toContain('katex-display');
    const bad = renderLatex('\\frac{a');
    expect(bad.html).toBe('');
    expect(bad.error).not.toBe('');
  });

  it('does not render untrusted commands', () => {
    const r = renderLatex('\\href{javascript:alert(1)}{x}');
    expect(r.html).not.toContain('href="javascript');
    expect(renderLatex('\\htmlClass{x}{y}').html).not.toContain('class="x"');
  });

  it('splits one formula per line but keeps environments together', () => {
    expect(splitFormulas('a+b\n\nc=d\n')).toEqual(['a+b', 'c=d']);
    const aligned = '\\begin{aligned}\nx &= 1 \\\\\ny &= 2\n\\end{aligned}';
    expect(splitFormulas(`${aligned}\nE=mc^2`)).toEqual([aligned, 'E=mc^2']);
    expect(splitFormulas('\\frac{a\n}{b}')).toEqual(['\\frac{a\n}{b}']);
    expect(splitFormulas('x = 1 \\\\\ny = 2')).toEqual(['x = 1 \\\\\ny = 2']);
    // 转义的花括号不算
    expect(splitFormulas('\\{ x \\}\ny')).toEqual(['\\{ x \\}', 'y']);
  });

  it('wraps each formula in $$', () => {
    expect(toDisplayMath('E=mc^2')).toBe('$$E=mc^2$$');
    expect(toDisplayMath('a\nb')).toBe('$$a$$\n\n$$b$$');
    expect(toDisplayMath('\\begin{aligned}\nx\n\\end{aligned}')).toBe('$$\n\\begin{aligned}\nx\n\\end{aligned}\n$$');
  });
});
