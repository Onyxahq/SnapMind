import { describe, expect, it } from 'vitest';
import { renderMarkdown } from './markdown';

describe('markdown preview', () => {
  it('renders common markdown', () => {
    const h = renderMarkdown('# 标题\n\n- 一\n- 二\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```\ncode <x>\n```');
    expect(h).toContain('<h1>标题</h1>');
    expect(h).toContain('<li>一</li>');
    expect(h).toContain('<table>');
    expect(h).toContain('code &lt;x&gt;');
  });

  it('escapes raw html but keeps sup / sub', () => {
    const h = renderMarkdown('x<sup>2</sup> H<sub>2</sub>O <img src=x onerror="alert(1)"> <script>alert(1)</script>');
    expect(h).toContain('<sup>2</sup>');
    expect(h).toContain('<sub>2</sub>');
    expect(h).not.toContain('<img');
    expect(h).not.toContain('<script');
    expect(h).toContain('&lt;script&gt;');
  });

  it('never makes navigable links or remote images', () => {
    const h = renderMarkdown('[点我](javascript:alert(1)) ![图](https://example.com/a.png)');
    expect(h).not.toContain('href');
    expect(h).not.toContain('<img');
    expect(h).toContain('title="javascript:alert(1)"');
    expect(h).toContain('［图片：图］');
  });

  it('supports obsidian extras', () => {
    const h = renderMarkdown('==重点== [[笔记|别名]] 价格 $5 和 $10，公式 $a^2$\n\n$$\nE=mc^2\n$$\n\n> [!tip] 提示\n> 内容');
    expect(h).toContain('<mark>重点</mark>');
    expect(h).toContain('<span class="wikilink" title="笔记">别名</span>');
    expect(h).toContain('价格 $5 和 $10');
    expect(h).toMatch(/<span class="math"><span class="katex">/);
    expect(h).toMatch(/<div class="math"><span class="katex-display">/);
    // 写错的公式按源码显示
    expect(renderMarkdown('$\\frac{a$')).toContain('<code class="math bad"');
    expect(h).toContain('class="callout" data-type="tip"');
    expect(h).toContain('<strong class="callout-title">提示</strong>');
  });
});
