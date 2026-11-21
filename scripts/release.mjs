#!/usr/bin/env node
// SnapMind 打包脚本：（可选）改版本号 → 测试 → 打 NSIS 安装包 → 静默安装到本机 → 校验。
//
// 用法：
//   pnpm release                 按当前版本打包并安装
//   pnpm release patch|minor|major|0.2.1   先改版本号再打包
// 选项：
//   --skip-tests   跳过 cargo test / pnpm test
//   --no-install   只打包，不安装
//   --launch       安装后从开始菜单快捷方式启动，确认能运行
//
// 版本号规则见 docs/进度.md「决策记录 · M0.5」。

import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const CARGO_TOML = join(ROOT, 'Cargo.toml');
const PACKAGE_JSON = join(ROOT, 'package.json');
const SIZE_BUDGET_MB = 25; // 编程计划 §6：安装包 ≤ 25 MB（不含模型）
const isWin = process.platform === 'win32';

// ---------------------------------------------------------------- 版本号

/** 读取 [workspace.package] 里的版本号（唯一来源）。 */
export function readVersion(toml) {
  const m = toml.match(/\[workspace\.package\][^[]*?\nversion\s*=\s*"([^"]+)"/);
  if (!m) throw new Error('Cargo.toml 里找不到 [workspace.package] version');
  return m[1];
}

/** 计算新版本号：patch / minor / major，或直接给出 x.y.z。 */
export function bumpVersion(current, how) {
  if (/^\d+\.\d+\.\d+$/.test(how)) return how;
  const [maj, min, pat] = current.split('.').map(Number);
  switch (how) {
    case 'patch': return `${maj}.${min}.${pat + 1}`;
    case 'minor': return `${maj}.${min + 1}.0`;
    case 'major': return `${maj + 1}.0.0`;
    default: throw new Error(`看不懂的版本参数：${how}`);
  }
}

/** 把新版本号写回 Cargo.toml（workspace）和 package.json，两处保持一致。 */
export function writeVersion(toml, pkgText, version) {
  const nextToml = toml.replace(
    /(\[workspace\.package\][^[]*?\nversion\s*=\s*")[^"]+(")/,
    `$1${version}$2`,
  );
  const pkg = JSON.parse(pkgText);
  pkg.version = version;
  return { toml: nextToml, pkg: JSON.stringify(pkg, null, 2) + '\n' };
}

// ---------------------------------------------------------------- 步骤

function run(cmd, args, opts = {}) {
  console.log(`\n▶ ${cmd} ${args.join(' ')}`);
  const r = spawnSync(cmd, args, { cwd: ROOT, stdio: 'inherit', shell: isWin, ...opts });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')} 失败（退出码 ${r.status}）`);
}

/** 运行一段 PowerShell 并返回输出。查无结果时 PowerShell 会返回非零退出码，这里统一视为成功。 */
function ps(script) {
  return execFileSync('powershell', ['-NoProfile', '-Command', `${script}; exit 0`], { encoding: 'utf8' }).trim();
}

const installDir = () => join(process.env.LOCALAPPDATA ?? join(homedir(), 'AppData', 'Local'), 'SnapMind');
const startMenuLink = () =>
  join(process.env.APPDATA ?? '', 'Microsoft', 'Windows', 'Start Menu', 'Programs', 'SnapMind.lnk');

/** 结束已安装版本的进程（只匹配安装目录里的 exe，连同 WebView2 子进程），以便覆盖安装。 */
function stopInstalled() {
  const exe = join(installDir(), 'snapmind.exe');
  const ids = ps(
    `Get-Process snapmind -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${exe}' } | ForEach-Object { $_.Id }`,
  );
  for (const id of ids.split(/\s+/).filter(Boolean)) {
    console.log(`  结束正在运行的 SnapMind（PID ${id}）`);
    spawnSync('taskkill', ['/PID', id, '/T', '/F'], { stdio: 'ignore' });
  }
}

function install(setup, version) {
  stopInstalled();
  console.log(`\n▶ 静默安装 ${setup}`);
  const r = spawnSync(setup, ['/S'], { stdio: 'inherit' });
  if (r.status !== 0) throw new Error(`安装程序退出码 ${r.status}`);
  const exe = join(installDir(), 'snapmind.exe');
  if (!existsSync(exe)) throw new Error(`安装后找不到 ${exe}`);
  const installed = ps(`(Get-Item '${exe}').VersionInfo.ProductVersion`);
  if (!installed.startsWith(version)) throw new Error(`已安装版本 ${installed}，期望 ${version}`);
  if (!existsSync(startMenuLink())) throw new Error('开始菜单里没有 SnapMind 快捷方式');
  console.log(`  已安装 ${installed} → ${installDir()}`);
}

function launchFromStartMenu() {
  console.log('\n▶ 从开始菜单快捷方式启动');
  spawnSync('cmd', ['/c', 'start', '""', startMenuLink()], { stdio: 'ignore' });
  const exe = join(installDir(), 'snapmind.exe');
  for (let i = 0; i < 20; i++) {
    const n = ps(`@(Get-Process snapmind -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq '${exe}' }).Count`);
    if (Number(n) > 0) return console.log('  SnapMind 已启动');
    spawnSync('powershell', ['-NoProfile', '-Command', 'Start-Sleep -Milliseconds 500']);
  }
  throw new Error('从开始菜单启动后 10 秒内没有看到 SnapMind 进程');
}

// ---------------------------------------------------------------- 主流程

function main(argv) {
  const flags = new Set(argv.filter((a) => a.startsWith('--')));
  const how = argv.find((a) => !a.startsWith('--'));

  let toml = readFileSync(CARGO_TOML, 'utf8');
  let version = readVersion(toml);
  if (how) {
    const next = bumpVersion(version, how);
    const out = writeVersion(toml, readFileSync(PACKAGE_JSON, 'utf8'), next);
    writeFileSync(CARGO_TOML, out.toml);
    writeFileSync(PACKAGE_JSON, out.pkg);
    console.log(`版本号 ${version} → ${next}`);
    version = next;
  }
  console.log(`SnapMind ${version}`);

  if (!flags.has('--skip-tests')) {
    run('cargo', ['test', '--workspace', '--quiet']);
    run('pnpm', ['test']);
  }
  run('pnpm', ['tauri', 'build']);

  if (!isWin) return console.log('\n非 Windows 平台：打包完成，跳过安装。');

  const setup = join(ROOT, 'target', 'release', 'bundle', 'nsis', `SnapMind_${version}_x64-setup.exe`);
  if (!existsSync(setup)) throw new Error(`找不到安装包 ${setup}`);
  const mb = statSync(setup).size / 1024 / 1024;
  console.log(`\n安装包：${setup}（${mb.toFixed(2)} MB）`);
  if (mb > SIZE_BUDGET_MB) console.warn(`⚠ 超出 ${SIZE_BUDGET_MB} MB 预算`);

  if (flags.has('--no-install')) return;
  install(setup, version);
  if (flags.has('--launch')) launchFromStartMenu();
  console.log('\n完成。');
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  try {
    main(process.argv.slice(2));
  } catch (e) {
    console.error(`\n✗ ${e.message}`);
    process.exit(1);
  }
}
