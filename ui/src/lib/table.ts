// 表格卡片（需求 3.8「表格」、M8.2）的数据：单元格按 HTML 表格的规则排列（一行里只列出从这一行开始的
// 单元格，被合并占住的位置不出现）。可以转成 Markdown / HTML / TSV（粘贴进 Excel），
// 也能把改过的 Markdown / HTML 源码解析回来。

export interface TCell {
  text: string;
  colspan: number;
  rowspan: number;
  header: boolean;
}
export type TRows = TCell[][];

/** 表格卡片的视图和状态 */
export type TableView = 'grid' | 'markdown' | 'html';
export interface TableCardState {
  id: string;
  /** run 识别中；model 需要先下载表格模型；done 完成；error 出错 */
  stage: 'run' | 'model' | 'done' | 'error';
  rows: TRows;
  meta: string;
  error: string;
}

/** 单元格摆到网格上：返回每个单元格的（行，列）以及总列数。 */
export function place(rows: TRows): { cells: { r: number; c: number; cell: TCell }[]; cols: number } {
  const taken = new Set<string>();
  const cells: { r: number; c: number; cell: TCell }[] = [];
  let cols = 0;
  rows.forEach((row, r) => {
    let c = 0;
    for (const cell of row) {
      while (taken.has(`${r},${c}`)) c++;
      const cs = Math.max(1, cell.colspan);
      const rs = Math.max(1, cell.rowspan);
      for (let dr = 0; dr < rs; dr++) for (let dc = 0; dc < cs; dc++) taken.add(`${r + dr},${c + dc}`);
      cells.push({ r, c, cell });
      c += cs;
      cols = Math.max(cols, c);
    }
  });
  return { cells, cols };
}

/** 展开成规整的二维文字表：合并单元格的文字放在左上角，其余位置为空。 */
export function expand(rows: TRows): string[][] {
  const { cells, cols } = place(rows);
  const height = Math.max(rows.length, ...cells.map((x) => x.r + Math.max(1, x.cell.rowspan)));
  const grid = Array.from({ length: height }, () => Array<string>(cols).fill(''));
  for (const { r, c, cell } of cells) grid[r][c] = cell.text;
  return grid;
}

const mdCell = (s: string) => s.replace(/\|/g, '\\|').replace(/\r?\n/g, '<br>');

/** Markdown 管道表格（不支持合并单元格：文字放在左上角那一格）。第一行作表头。 */
export function toMarkdown(rows: TRows): string {
  const g = expand(rows);
  if (!g.length) return '';
  const line = (r: string[]) => `| ${r.map(mdCell).join(' | ')} |`;
  return [line(g[0]), `| ${g[0].map(() => '---').join(' | ')} |`, ...g.slice(1).map(line)].join('\n');
}

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/** HTML 表格（保留合并单元格和表头）。 */
export function toHtml(rows: TRows): string {
  const tr = (row: TCell[]) =>
    `  <tr>${row
      .map((c) => {
        const tag = c.header ? 'th' : 'td';
        const span = `${c.colspan > 1 ? ` colspan="${c.colspan}"` : ''}${c.rowspan > 1 ? ` rowspan="${c.rowspan}"` : ''}`;
        return `<${tag}${span}>${esc(c.text).replace(/\n/g, '<br>')}</${tag}>`;
      })
      .join('')}</tr>`;
  const head = rows.filter((r) => r.length && r.every((c) => c.header));
  const body = rows.slice(head.length);
  const parts = ['<table>'];
  if (head.length) parts.push('<thead>', ...head.map(tr), '</thead>');
  parts.push('<tbody>', ...body.map(tr), '</tbody>', '</table>');
  return parts.join('\n');
}

/** TSV：粘贴进 Excel / 表格软件时自动分格。单元格里的换行和制表符换成空格。 */
export function toTsv(rows: TRows): string {
  return expand(rows)
    .map((r) => r.map((s) => s.replace(/[\t\r\n]+/g, ' ')).join('\t'))
    .join('\n');
}

/** 解析 Markdown 管道表格；没有表格时返回 null。 */
export function parseMarkdown(md: string): TRows | null {
  const lines = md.split(/\r?\n/).map((l) => l.trim()).filter((l) => l.startsWith('|') || /\S\s*\|\s*\S/.test(l));
  if (!lines.length) return null;
  const split = (l: string) =>
    l
      .replace(/^\||\|$/g, '')
      .split(/(?<!\\)\|/)
      .map((s) => s.trim().replace(/\\\|/g, '|').replace(/<br\s*\/?>/gi, '\n'));
  const rows: TRows = [];
  lines.forEach((l, i) => {
    const cells = split(l);
    if (cells.every((s) => /^:?-{2,}:?$/.test(s))) return; // 分隔行
    rows.push(cells.map((text) => ({ text, colspan: 1, rowspan: 1, header: i === 0 })));
  });
  return rows.length ? rows : null;
}

const decode = (s: string) =>
  s
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, '&')
    .trim();

/** 解析 HTML 表格（大模型的输出或改过的 HTML 源码）；没有表格时返回 null。 */
export function parseHtml(html: string): TRows | null {
  const rows: TRows = [];
  let inHead = false;
  const re = /<(\/?)(thead|tr|td|th)\b([^>]*)>|([^<]+|<(?!\/?(?:thead|tr|td|th)\b)[^>]*>)/gi;
  let cell: TCell | null = null;
  let buf = '';
  for (let m = re.exec(html); m; m = re.exec(html)) {
    const [, close, tag, attrs, text] = m;
    if (text !== undefined) {
      if (cell) buf += text;
      continue;
    }
    const t = tag.toLowerCase();
    if (t === 'thead') inHead = !close;
    else if (t === 'tr' && !close) rows.push([]);
    else if ((t === 'td' || t === 'th') && !close) {
      if (!rows.length) rows.push([]);
      const num = (name: string) => Number(new RegExp(`${name}\\s*=\\s*["']?(\\d+)`, 'i').exec(attrs)?.[1] ?? 1) || 1;
      cell = { text: '', colspan: num('colspan'), rowspan: num('rowspan'), header: inHead || t === 'th' };
      buf = '';
    } else if ((t === 'td' || t === 'th') && close && cell) {
      cell.text = decode(buf);
      rows[rows.length - 1].push(cell);
      cell = null;
    }
  }
  const out = rows.filter((r) => r.length);
  return out.length ? out : null;
}
