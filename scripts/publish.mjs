#!/usr/bin/env node
// 发布新版本到 GitHub Releases（M11.3）：已安装的 SnapMind 从这里检查和下载更新。
//
// 先 `pnpm release …` 打出带签名的安装包（需要本机有更新签名私钥），提交并推送，再运行：
//   pnpm publish-release [--notes "这次的改动"] [--allow-private]
//
// 上传：安装包、安装包签名（.sig）、latest.json（写入说明）。标签 v<版本> 打在当前提交上。
// 说明默认取当前提交标题（去掉 feat(M11.3): 这样的前缀）。仓库是私有的时候用户拿不到更新，默认拒绝发布。

import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { RELEASE_REPO, readVersion, setupName } from './release.mjs';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');

/** 提交标题 → 给用户看的说明：去掉 conventional commit 前缀。 */
export function notesFromSubject(subject) {
  return subject.replace(/^[a-z]+(\([^)]*\))?!?:\s*/i, '').trim();
}

function sh(cmd, args) {
  return execFileSync(cmd, args, { cwd: ROOT, encoding: 'utf8' }).trim();
}

function main(argv) {
  const at = argv.indexOf('--notes');
  const version = readVersion(readFileSync(join(ROOT, 'Cargo.toml'), 'utf8'));
  const nsis = join(ROOT, 'target', 'release', 'bundle', 'nsis');
  const setup = join(nsis, setupName(version));
  const sig = `${setup}.sig`;
  const manifest = join(nsis, 'latest.json');
  for (const f of [setup, sig, manifest]) {
    if (!existsSync(f)) throw new Error(`找不到 ${f}：先用带更新签名私钥的 pnpm release 打包`);
  }

  if (JSON.parse(sh('gh', ['repo', 'view', RELEASE_REPO, '--json', 'isPrivate'])).isPrivate && !argv.includes('--allow-private')) {
    throw new Error(`${RELEASE_REPO} 是私有仓库，已安装的 SnapMind 下载不到更新。仓库公开后再发布（或加 --allow-private 只做存档）`);
  }
  if (sh('git', ['status', '--porcelain'])) throw new Error('工作区有未提交的改动：先提交并推送');

  const notes = at >= 0 ? argv[at + 1] : notesFromSubject(sh('git', ['log', '-1', '--format=%s']));
  const m = JSON.parse(readFileSync(manifest, 'utf8'));
  if (m.version !== version) throw new Error(`latest.json 是 ${m.version}，当前版本 ${version}：重新打包`);
  m.notes = notes;
  writeFileSync(manifest, JSON.stringify(m, null, 2) + '\n');

  const target = sh('git', ['rev-parse', 'HEAD']);
  console.log(`发布 v${version}（${target.slice(0, 7)}）到 ${RELEASE_REPO}：${notes}`);
  execFileSync(
    'gh',
    ['release', 'create', `v${version}`, setup, sig, manifest, '--repo', RELEASE_REPO, '--target', target, '--title', `SnapMind ${version}`, '--notes', notes],
    { cwd: ROOT, stdio: 'inherit' },
  );
  console.log('完成。');
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(`\n✗ ${e.message}`);
    process.exit(1);
  }
}
