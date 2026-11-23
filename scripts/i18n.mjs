// 多语言词典维护工具。源语言是简体中文，键就是中文原文（见 ui/src/lib/i18n.svelte.ts、
// crates/snapmind-core/src/i18n.rs）。词典分两份：
//   ui/src/locales/<语言>.json            界面（t('…') / msg('…')）
//   crates/snapmind-core/locales/<语言>.json  后端（tr("…") / trf("…", …)）
//
//   node scripts/i18n.mjs check          检查缺译、占位符不一致、没用到的旧键（缺译时退出码 1）
//   node scripts/i18n.mjs todo [ui|rs]   列出英文词典里缺的键（JSON 数组），翻译时用
//   node scripts/i18n.mjs apply 文件.jsonl  合并译文：每行 [中文, en, zh-TW, ja, ko, es, fr, de, ru, pt-BR]
//   node scripts/i18n.mjs prune          删掉没用到的旧键
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
/** 译文语言（zh-CN 是源语言，没有词典），顺序和 apply 的列顺序一致 */
export const LOCALES = ['en', 'zh-TW', 'ja', 'ko', 'es', 'fr', 'de', 'ru', 'pt-BR'];
const DIRS = { ui: 'ui/src/locales', rs: 'crates/snapmind-core/locales' };
const CJK = /[\u3400-\u9fff]/;

function walk(dir, exts, out = []) {
  for (const e of fs.readdirSync(path.join(root, dir), { withFileTypes: true })) {
    const rel = `${dir}/${e.name}`;
    if (e.isDirectory()) {
      if (['node_modules', 'target', 'dist', 'locales', 'tests', 'fixtures', 'gallery'].includes(e.name)) continue;
      walk(rel, exts, out);
    } else if (exts.some((x) => e.name.endsWith(x)) && !/\.test\.ts$/.test(e.name)) out.push(rel);
  }
  return out;
}

const read = (rel) => fs.readFileSync(path.join(root, rel), 'utf8');

function unescapeJs(s) {
  return s.replace(/\\(u[0-9a-fA-F]{4}|x[0-9a-fA-F]{2}|[\s\S])/g, (_, c) => {
    if (c[0] === 'u' || c[0] === 'x') return String.fromCharCode(parseInt(c.slice(1), 16));
    return { n: '\n', t: '\t', r: '\r' }[c] ?? c;
  });
}

/** 去掉 Rust 的 // 注释（粗略，字符串里的 // 不会误伤带引号的行首之外的情况） */
const stripRsComments = (src) => {
  const cut = src.indexOf('#[cfg(test)]'); // 测试模块不算
  return (cut < 0 ? src : src.slice(0, cut)).replace(/^\s*\/\/.*$/gm, '').replace(/\s\/\/ .*$/gm, '');
};

/** 收集代码里用到的键：{ ui: Set, rs: Set } */
export function collectKeys() {
  const ui = new Set();
  const rs = new Set();
  const re = /(?<![\w.$])(?:t|msg)\(\s*(['"])((?:\\.|(?!\1)[^\\\n])*)\1/g;
  for (const f of walk('ui/src', ['.svelte', '.ts'])) {
    if (f.endsWith('lib/i18n.svelte.ts')) continue;
    for (const m of read(f).matchAll(re)) ui.add(unescapeJs(m[2]));
  }
  const rsRe = /(?<![\w.])trf?\(\s*"((?:\\.|[^"\\])*)"/g;
  for (const dir of ['src-tauri/src', 'crates']) {
    for (const f of walk(dir, ['.rs'])) {
      for (const m of stripRsComments(read(f)).matchAll(rsRe)) rs.add(unescapeJs(m[1]));
    }
  }
  // 后端数据里直接带的中文（前端用 t() 翻译，后端用 tr() 翻译）：语言名、模型服务商简介
  const both = (file, pick) => {
    for (const m of stripRsComments(read(file)).matchAll(pick)) {
      const text = unescapeJs(m[1]);
      if (CJK.test(text)) {
        ui.add(text);
        rs.add(text);
      }
    }
  };
  both('crates/snapmind-core/src/lang.rs', /name: "((?:\\.|[^"\\])*)"/g);
  const cat = stripRsComments(read('crates/snapmind-providers/src/catalog.rs'));
  for (const m of cat.matchAll(/\b(?:name|desc): "((?:\\.|[^"\\])*)"/g)) {
    const text = unescapeJs(m[1]);
    if (CJK.test(text)) ui.add(text);
  }
  return { ui, rs };
}

export function loadLocale(side, locale) {
  const p = path.join(root, DIRS[side], `${locale}.json`);
  return fs.existsSync(p) ? JSON.parse(fs.readFileSync(p, 'utf8')) : {};
}

function saveLocale(side, locale, dict) {
  const sorted = Object.fromEntries(Object.entries(dict).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
  const p = path.join(root, DIRS[side], `${locale}.json`);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, JSON.stringify(sorted, null, 2) + '\n');
}

const placeholders = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(',');

/** 检查所有词典：缺译、占位符对不上、没用到的旧键 */
export function checkLocales() {
  const keys = collectKeys();
  const problems = { missing: [], mismatch: [], stale: [] };
  for (const side of ['ui', 'rs']) {
    for (const locale of LOCALES) {
      const dict = loadLocale(side, locale);
      for (const k of keys[side]) {
        const v = dict[k];
        if (typeof v !== 'string' || v.trim() === '') problems.missing.push(`${side}/${locale}: ${k}`);
        else if (placeholders(v) !== placeholders(k)) problems.mismatch.push(`${side}/${locale}: ${k} → ${v}`);
      }
      for (const k of Object.keys(dict)) if (!keys[side].has(k)) problems.stale.push(`${side}/${locale}: ${k}`);
    }
  }
  return { keys, problems };
}

function cmdTodo(side) {
  const keys = collectKeys();
  const out = [];
  for (const s of side ? [side] : ['ui', 'rs']) {
    const en = loadLocale(s, 'en');
    for (const k of keys[s]) if (!en[k]) out.push({ side: s, key: k });
  }
  console.log(JSON.stringify(out.map((o) => o.key), null, 0));
  console.error(`${out.length} 条待翻译`);
}

function cmdApply(file) {
  const keys = collectKeys();
  const rows = fs
    .readFileSync(file, 'utf8')
    .split('\n')
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l, i) => {
      try {
        return JSON.parse(l);
      } catch (e) {
        throw new Error(`第 ${i + 1} 行不是合法 JSON：${l.slice(0, 60)}`);
      }
    });
  const dicts = {};
  for (const side of ['ui', 'rs']) for (const l of LOCALES) dicts[`${side}/${l}`] = loadLocale(side, l);
  let added = 0;
  for (const row of rows) {
    if (row.length !== LOCALES.length + 1) throw new Error(`列数不对（${row.length}）：${String(row[0]).slice(0, 40)}`);
    const [zh, ...tr] = row;
    const sides = ['ui', 'rs'].filter((s) => keys[s].has(zh));
    if (!sides.length) {
      console.error('没有用到，跳过：' + zh.slice(0, 50));
      continue;
    }
    for (const s of sides) LOCALES.forEach((l, i) => (dicts[`${s}/${l}`][zh] = tr[i]));
    added++;
  }
  for (const side of ['ui', 'rs']) for (const l of LOCALES) saveLocale(side, l, dicts[`${side}/${l}`]);
  console.log(`合并 ${added} 条`);
}

function cmdPrune() {
  const keys = collectKeys();
  let n = 0;
  for (const side of ['ui', 'rs']) {
    for (const l of LOCALES) {
      const d = loadLocale(side, l);
      for (const k of Object.keys(d)) if (!keys[side].has(k)) (delete d[k], n++);
      saveLocale(side, l, d);
    }
  }
  console.log(`删掉 ${n} 个旧键`);
}

function cmdCheck() {
  const { keys, problems } = checkLocales();
  console.log(`界面 ${keys.ui.size} 条，后端 ${keys.rs.size} 条，译文 ${LOCALES.length} 种`);
  for (const [name, list] of Object.entries(problems)) {
    if (!list.length) continue;
    console.log(`\n${name}（${list.length}）`);
    for (const x of list.slice(0, 40)) console.log('  ' + x);
    if (list.length > 40) console.log(`  …还有 ${list.length - 40} 条`);
  }
  process.exit(problems.missing.length || problems.mismatch.length ? 1 : 0);
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [cmd, arg] = process.argv.slice(2);
  if (cmd === 'check') cmdCheck();
  else if (cmd === 'todo') cmdTodo(arg);
  else if (cmd === 'apply') cmdApply(arg);
  else if (cmd === 'prune') cmdPrune();
  else console.log('用法：node scripts/i18n.mjs check | todo [ui|rs] | apply 文件.jsonl | prune');
}
