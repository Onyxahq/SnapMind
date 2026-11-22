// 文件名模板的预览（设置 → 截图与贴图）。规则和 snapmind-core::export::render_filename 一致：
// 花括号里的 yyyy MM dd HH mm ss SSS 换成时间，其他字符原样保留；不合法的文件名字符换成下划线。

const pad = (n: number, w: number) => String(n).padStart(w, '0');

export function renderFilename(template: string, now: Date): string {
  const tokens: [string, string][] = [
    ['yyyy', pad(now.getFullYear(), 4)],
    ['SSS', pad(now.getMilliseconds(), 3)],
    ['MM', pad(now.getMonth() + 1, 2)],
    ['dd', pad(now.getDate(), 2)],
    ['HH', pad(now.getHours(), 2)],
    ['mm', pad(now.getMinutes(), 2)],
    ['ss', pad(now.getSeconds(), 2)],
  ];
  let out = '';
  let rest = template;
  for (;;) {
    const open = rest.indexOf('{');
    if (open < 0) break;
    out += rest.slice(0, open);
    const after = rest.slice(open + 1);
    const close = after.indexOf('}');
    if (close < 0) {
      out += rest.slice(open);
      rest = '';
      break;
    }
    let inner = after.slice(0, close);
    while (inner) {
      const t = tokens.find(([k]) => inner.startsWith(k));
      if (t) {
        out += t[1];
        inner = inner.slice(t[0].length);
      } else {
        out += inner[0];
        inner = inner.slice(1);
      }
    }
    rest = after.slice(close + 1);
  }
  out += rest;
  return sanitize(out);
}

function sanitize(name: string): string {
  // eslint-disable-next-line no-control-regex
  const s = name.replace(/[<>:"/\\|?*\x00-\x1f]/g, '_').replace(/[. ]+$/, '');
  return s.trim() ? s : 'SnapMind';
}
