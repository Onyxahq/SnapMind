// 从 models.dev（开源模型目录，MIT）生成「模型能不能看图 / 会不会推理」的精简索引，随程序打包。
//
// 用法：node scripts/vision-index.mjs [api.json 的本地路径]
//   不给路径时从 https://models.dev/api.json 下载。发版前跑一次，让新出的模型也能认出来。
//
// 输出 crates/snapmind-providers/data/models-dev.tsv：每行「模型名<TAB>标记」，模型名是去掉组织前缀、
// 冒号后缀并转小写的写法（openai/gpt-4o:free → gpt-4o），标记里 v = 能看图、r = 推理模型。
// 同一个模型名在多家服务商上说法不一时，先听模型厂商自己的，再按多数。
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(ROOT, 'crates', 'snapmind-providers', 'data', 'models-dev.tsv');

/** 模型厂商自己的条目（说法冲突时以它们为准）。 */
const VENDORS = new Set([
  'openai', 'anthropic', 'google', 'xai', 'mistral', 'deepseek', 'moonshotai', 'moonshotai-cn', 'zhipuai', 'zai',
  'alibaba', 'alibaba-cn', 'minimax', 'minimax-cn', 'stepfun', 'stepfun-ai', 'xiaomi', 'meta', 'cohere', 'volcengine',
  'byteplus', 'tencent-tokenhub', 'longcat', 'thinkingmachines', 'sakana', 'ai21', 'upstage', 'perplexity', 'amazon-bedrock',
]);

/** 列表里的 id 和这里的写法对齐：转小写，去掉 `组织/` 前缀、`~` 前缀和 `:free` 之类的后缀。 */
export function bareId(id) {
  return id.toLowerCase().split('/').pop().replace(/^~/, '').replace(/:.*$/, '');
}

export function buildIndex(api) {
  /** id → { vendor: flags | null, votes: Map<flags, n> } */
  const acc = new Map();
  for (const [prov, p] of Object.entries(api)) {
    for (const [id, m] of Object.entries(p.models ?? {})) {
      const input = m.modalities?.input;
      if (!Array.isArray(input)) continue;
      const flags = (input.includes('image') ? 'v' : '') + (m.reasoning ? 'r' : '');
      const key = bareId(id);
      if (!key) continue;
      const e = acc.get(key) ?? { vendor: null, votes: new Map() };
      if (VENDORS.has(prov) && e.vendor === null) e.vendor = flags;
      e.votes.set(flags, (e.votes.get(flags) ?? 0) + 1);
      acc.set(key, e);
    }
  }
  const rows = [];
  for (const [id, e] of [...acc].sort(([a], [b]) => (a < b ? -1 : 1))) {
    // 票数相同时听说「能看图 / 会推理」的那家（有的托管方只开放了文字输入）
    const flags = e.vendor ?? [...e.votes].sort((a, b) => b[1] - a[1] || b[0].length - a[0].length || (a[0] < b[0] ? -1 : 1))[0][0];
    rows.push(`${id}\t${flags}`);
  }
  return rows;
}

async function main() {
  const src = process.argv[2];
  const text = src ? readFileSync(src, 'utf8') : await (await fetch('https://models.dev/api.json')).text();
  const rows = buildIndex(JSON.parse(text));
  const date = new Date().toISOString().slice(0, 10);
  const head = [
    `# 由 scripts/vision-index.mjs 从 models.dev/api.json 生成（${date}，models.dev 以 MIT 许可发布）。`,
    '# 每行：模型名（去掉组织前缀和冒号后缀，小写）<TAB>标记（v = 能看图，r = 推理模型）。不要手改。',
  ];
  writeFileSync(OUT, [...head, ...rows, ''].join('\n'), 'utf8');
  const vision = rows.filter((r) => r.split('\t')[1].includes('v')).length;
  console.log(`写入 ${OUT}：${rows.length} 个模型，其中 ${vision} 个能看图`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
