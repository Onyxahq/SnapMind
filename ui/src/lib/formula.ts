// 公式（需求 3.8「公式 → LaTeX」、M8.3）：KaTeX 渲染、按公式拆分、复制成 $$…$$。
//
// 内容来自大模型，渲染在能调用 Rust 命令的页面里：KaTeX 的 trust 保持关闭（\href、\url、
// \includegraphics、\htmlClass 之类不渲染），输出的是 KaTeX 自己生成并转义过的 HTML。
import katex from 'katex';
import 'katex/dist/katex.min.css';

export interface Rendered {
  html: string;
  /** 渲染失败时的错误（html 为空） */
  error: string;
}

/** 渲染一段 LaTeX；失败时返回错误信息而不是抛出。 */
export function renderLatex(tex: string, display = true): Rendered {
  try {
    const html = katex.renderToString(tex, {
      displayMode: display,
      throwOnError: true,
      strict: 'ignore',
      trust: false,
      output: 'html',
      maxExpand: 500,
    });
    return { html, error: '' };
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    return { html: '', error: msg.replace(/^KaTeX parse error:\s*/, '') };
  }
}

/**
 * 拆成独立的公式：每行一个（提示词要求），但 \begin…\end 环境、没配平的花括号、
 * 以 \\ 结尾的行会和下一行连在一起；空行丢掉。
 */
export function splitFormulas(src: string): string[] {
  const out: string[] = [];
  let cur: string[] = [];
  let env = 0;
  let brace = 0;
  for (const line of src.split('\n')) {
    if (!line.trim() && !cur.length) continue;
    cur.push(line);
    const code = line.replace(/\\[{}%]/g, '').replace(/%.*$/, '');
    env += (code.match(/\\begin\{/g)?.length ?? 0) - (code.match(/\\end\{/g)?.length ?? 0);
    for (const ch of code) brace += ch === '{' ? 1 : ch === '}' ? -1 : 0;
    if (env <= 0 && brace <= 0 && !/\\\\\s*$/.test(code)) {
      const f = cur.join('\n').trim();
      if (f) out.push(f);
      cur = [];
      env = brace = 0;
    }
  }
  const rest = cur.join('\n').trim();
  if (rest) out.push(rest);
  return out;
}

/** 复制成 $$…$$：每个公式各包一对，单行的写在一行里，多行的定界符单独占行；公式之间空一行。 */
export function toDisplayMath(src: string): string {
  return splitFormulas(src)
    .map((f) => (f.includes('\n') ? `$$\n${f}\n$$` : `$$${f}$$`))
    .join('\n\n');
}
