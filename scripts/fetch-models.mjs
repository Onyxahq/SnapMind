// 下载本地 OCR 模型（PP-OCRv5 mobile 检测 + 识别，PaddlePaddle 官方 ONNX，Apache-2.0）到 models/ppocrv5/。
//
// 用法：node scripts/fetch-models.mjs [--force]
//
// - 地址固定到 Hugging Face 仓库的某个提交，内容不会悄悄变化；
// - 每个文件都校验 SHA-256，不符就删除并报错；已存在且校验通过的文件跳过；
// - models/ 不入库（见 .gitignore），打包时作为资源一起装进安装包。
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUT = join(ROOT, 'models', 'ppocrv5');

const hf = (repo, rev, file) => `https://huggingface.co/PaddlePaddle/${repo}/resolve/${rev}/${file}`;
const DET = ['PP-OCRv5_mobile_det_onnx', 'e6f4fa85f00e168c862bc462aebca69eef9b3d3d'];
const REC = ['PP-OCRv5_mobile_rec_onnx', 'ed152b8b495f84de93cda5709d768548a9127622'];

export const FILES = [
  { name: 'det.onnx', url: hf(...DET, 'inference.onnx'), sha256: 'a431985659dc921974177a95adcfbb90fd9e51989a5e04d70d0b75f597b6e61d' },
  { name: 'det.yml', url: hf(...DET, 'inference.yml'), sha256: '98069072e1b6b37d727fd9d9f11725faa46d6ea0de012f2ed26caea011c37699' },
  { name: 'rec.onnx', url: hf(...REC, 'inference.onnx'), sha256: 'da72dc72ca4dc220df0dfde68c1dedc31c58d3e76a25871122e5056227d50092' },
  { name: 'rec.yml', url: hf(...REC, 'inference.yml'), sha256: '5dfeb2777f6d0db8177d8128a8acfcf6e6276dc4ac73ea3bf0dc06d6a5e85d8e' },
];

const sha256 = (buf) => createHash('sha256').update(buf).digest('hex');

async function fetchOne(f, force) {
  const path = join(OUT, f.name);
  if (!force && existsSync(path) && sha256(readFileSync(path)) === f.sha256) {
    console.log(`  ✓ ${f.name}（已存在）`);
    return;
  }
  process.stdout.write(`  ↓ ${f.name} … `);
  const res = await fetch(f.url, { redirect: 'follow' });
  if (!res.ok) throw new Error(`${f.name}：HTTP ${res.status}`);
  const buf = Buffer.from(await res.arrayBuffer());
  const got = sha256(buf);
  if (got !== f.sha256) throw new Error(`${f.name}：SHA-256 不符\n  期望 ${f.sha256}\n  实际 ${got}`);
  writeFileSync(`${path}.part`, buf);
  renameSync(`${path}.part`, path);
  console.log(`${(buf.length / 1024 / 1024).toFixed(1)} MB`);
}

/** 从识别模型的 inference.yml 里取出字符表（PostProcess.character_dict），写成一行一个字符。 */
export function extractKeys(yml) {
  const lines = yml.split(/\r?\n/);
  const start = lines.findIndex((l) => /^\s*character_dict:\s*$/.test(l));
  if (start < 0) throw new Error('rec.yml 里没有 character_dict');
  const indent = lines[start + 1].match(/^(\s*)- /)?.[1] ?? '';
  const keys = [];
  for (const l of lines.slice(start + 1)) {
    if (!l.startsWith(`${indent}- `)) break;
    let v = l.slice(indent.length + 2);
    // YAML 标量：'x'（'' 表示 '）、"x"（反斜杠转义）或裸值
    if (v.startsWith("'") && v.endsWith("'")) v = v.slice(1, -1).replace(/''/g, "'");
    else if (v.startsWith('"') && v.endsWith('"')) v = JSON.parse(v);
    keys.push(v);
  }
  return keys;
}

async function main() {
  const force = process.argv.includes('--force');
  mkdirSync(OUT, { recursive: true });
  console.log(`下载 PP-OCRv5 mobile 模型到 ${OUT}`);
  for (const f of FILES) {
    try {
      await fetchOne(f, force);
    } catch (e) {
      rmSync(join(OUT, `${f.name}.part`), { force: true });
      console.error(`\n✗ ${e.message}`);
      process.exit(1);
    }
  }
  const keys = extractKeys(readFileSync(join(OUT, 'rec.yml'), 'utf8'));
  writeFileSync(join(OUT, 'keys.txt'), keys.join('\n'), 'utf8');
  console.log(`  ✓ keys.txt（${keys.length} 个字符）`);
  console.log('完成。');
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
