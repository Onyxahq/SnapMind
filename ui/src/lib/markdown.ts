// Markdown 预览（需求 3.8「Markdown」、M8.1）：默认 Obsidian 方言。
//
// 内容来自大模型或 OCR，渲染在有调用 Rust 权限的页面里，所以按「不信任」处理：
// - 原样 HTML 一律转义，只放行 <sup> <sub> <br> <mark> <kbd>（提示词要求上下标用 HTML）；
// - 链接不生成 href（点了不会让页面跳走），地址放在 title 里；图片只显示替代文字，不去网上取图。
// Obsidian 扩展：==高亮==、[[双链|别名]]、$行内公式$ / $$公式块$$（KaTeX 渲染，写错的按源码显示）、> [!note] 提示块。
import { Marked, type TokenizerAndRendererExtension, type Tokens } from 'marked';
import { renderLatex } from './formula';

const esc = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

const ALLOWED_TAG = /^<\/?(sup|sub|br|mark|kbd)\s*\/?>$/i;

const highlight: TokenizerAndRendererExtension = {
  name: 'highlight',
  level: 'inline',
  start: (src) => src.indexOf('=='),
  tokenizer(src) {
    const m = /^==(?=\S)([\s\S]*?\S)==/.exec(src);
    if (m) return { type: 'highlight', raw: m[0], text: m[1], tokens: this.lexer.inlineTokens(m[1]) };
  },
  renderer(t) {
    return `<mark>${this.parser.parseInline((t as Tokens.Generic).tokens ?? [])}</mark>`;
  },
};

const wikilink: TokenizerAndRendererExtension = {
  name: 'wikilink',
  level: 'inline',
  start: (src) => src.indexOf('[['),
  tokenizer(src) {
    const m = /^\[\[([^\]|\n]+)(?:\|([^\]\n]+))?\]\]/.exec(src);
    if (m) return { type: 'wikilink', raw: m[0], target: m[1].trim(), text: (m[2] ?? m[1]).trim() };
  },
  renderer(t) {
    return `<span class="wikilink" title="${esc(t.target)}">${esc(t.text)}</span>`;
  },
};

const mathBlock: TokenizerAndRendererExtension = {
  name: 'mathBlock',
  level: 'block',
  start: (src) => src.indexOf('$$'),
  tokenizer(src) {
    const m = /^\$\$([\s\S]+?)\$\$[ \t]*(?:\n|$)/.exec(src);
    if (m) return { type: 'mathBlock', raw: m[0], text: m[1].trim() };
  },
  renderer(t) {
    const r = renderLatex(t.text, true);
    return r.html ? `<div class="math">${r.html}</div>\n` : `<pre class="math bad" title="${esc(r.error)}">${esc(t.text)}</pre>\n`;
  },
};

const mathInline: TokenizerAndRendererExtension = {
  name: 'mathInline',
  level: 'inline',
  start: (src) => src.indexOf('$'),
  tokenizer(src) {
    // 不把价格（$5 和 $10）当公式：$ 后面不是空白、结尾 $ 后面不是数字
    const m = /^\$(?!\s)([^$\n]+?)(?<!\s)\$(?!\d)/.exec(src);
    if (m) return { type: 'mathInline', raw: m[0], text: m[1] };
  },
  renderer(t) {
    const r = renderLatex(t.text, false);
    return r.html ? `<span class="math">${r.html}</span>` : `<code class="math bad" title="${esc(r.error)}">${esc(t.text)}</code>`;
  },
};

const marked = new Marked({ gfm: true, breaks: false });
marked.use({
  extensions: [mathBlock, mathInline, highlight, wikilink],
  renderer: {
    html(t: Tokens.HTML | Tokens.Tag) {
      const raw = t.text.trim();
      return ALLOWED_TAG.test(raw) ? raw : esc(t.text);
    },
    link(t: Tokens.Link) {
      return `<a class="link" title="${esc(t.href)}">${this.parser.parseInline(t.tokens)}</a>`;
    },
    image(t: Tokens.Image) {
      return `<span class="img" title="${esc(t.href)}">［图片${t.text ? `：${esc(t.text)}` : ''}］</span>`;
    },
  },
});

/** Obsidian 提示块：> [!note] 标题 → 带类型的引用块。 */
function callouts(html: string): string {
  return html.replace(
    /<blockquote>\s*<p>\[!(\w+)\][ \t]*([^\n<]*)/g,
    (_, type: string, title: string) =>
      `<blockquote class="callout" data-type="${esc(type.toLowerCase())}"><p><strong class="callout-title">${title.trim() || esc(type)}</strong>`,
  );
}

/** 把 Markdown 渲染成可以安全放进页面的 HTML。 */
export function renderMarkdown(src: string): string {
  return callouts(marked.parse(src, { async: false }) as string);
}
