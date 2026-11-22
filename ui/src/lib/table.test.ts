import { describe, expect, it } from 'vitest';
import { expand, parseHtml, parseMarkdown, place, toHtml, toMarkdown, toTsv, type TRows } from './table';

const c = (text: string, colspan = 1, rowspan = 1, header = false) => ({ text, colspan, rowspan, header });

const sample: TRows = [
  [c('名称', 1, 1, true), c('数量', 1, 1, true), c('备注', 1, 1, true)],
  [c('苹果'), c('3'), c('红色 | 甜', 1, 2)],
  [c('梨'), c('5')],
  [c('合计', 2), c('')],
];

describe('table model', () => {
  it('places cells around spans and expands', () => {
    const p = place(sample);
    expect(p.cols).toBe(3);
    expect(p.cells.find((x) => x.cell.text === '梨')).toMatchObject({ r: 2, c: 0 });
    expect(expand(sample)).toEqual([
      ['名称', '数量', '备注'],
      ['苹果', '3', '红色 | 甜'],
      ['梨', '5', ''],
      ['合计', '', ''],
    ]);
  });

  it('converts to markdown, html and tsv', () => {
    expect(toMarkdown(sample)).toBe(
      '| 名称 | 数量 | 备注 |\n| --- | --- | --- |\n| 苹果 | 3 | 红色 \\| 甜 |\n| 梨 | 5 |  |\n| 合计 |  |  |',
    );
    const h = toHtml(sample);
    expect(h).toContain('<thead>\n  <tr><th>名称</th><th>数量</th><th>备注</th></tr>\n</thead>');
    expect(h).toContain('<td rowspan="2">红色 | 甜</td>');
    expect(h).toContain('<td colspan="2">合计</td>');
    expect(toTsv(sample).split('\n')[1]).toBe('苹果\t3\t红色 | 甜');
  });

  it('round-trips html with spans and markdown', () => {
    expect(parseHtml(toHtml(sample))).toEqual(sample);
    const md = parseMarkdown(toMarkdown(sample))!;
    expect(md[0].every((x) => x.header)).toBe(true);
    expect(md[1].map((x) => x.text)).toEqual(['苹果', '3', '红色 | 甜']);
    expect(parseMarkdown('没有表格')).toBeNull();
  });

  it('parses model output html', () => {
    const rows = parseHtml('```html\n<table><tr><td>A &amp; B</td><td><b>粗</b><br>换行</td></tr></table>\n```')!;
    expect(rows).toEqual([[c('A & B'), c('粗\n换行')]]);
    expect(parseHtml('plain text')).toBeNull();
  });
});
