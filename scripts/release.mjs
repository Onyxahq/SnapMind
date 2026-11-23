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
// 自动更新（M11.3）：找到更新签名私钥时（环境变量 TAURI_SIGNING_PRIVATE_KEY，或
// ~/.tauri/snapmind-updater.key），顺带生成签名（.sig）和 latest.json，供 `pnpm publish-release` 上传。
// 私钥只放在本机、不进仓库；公钥写在 src-tauri/tauri.conf.json 的 plugins.updater.pubkey。
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
const TAURI_CONF = join(ROOT, 'src-tauri', 'tauri.conf.json');
/** 更新包放在这个仓库的 Releases 上（和 tauri.conf.json 里的 endpoints 一致）。 */
export const RELEASE_REPO = 'Onyxahq/SnapMind';
export const KEY_FILE = join(homedir(), '.tauri', 'snapmind-updater.key');
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

// ---------------------------------------------------------------- 更新清单

export const setupName = (version) => `SnapMind_${version}_x64-setup.exe`;

/** 更新源 latest.json 的内容（tauri-plugin-updater 的静态清单格式）。 */
export function updaterManifest({ version, signature, notes = '', date, repo = RELEASE_REPO }) {
  return {
    version,
    notes,
    pub_date: date.toISOString(),
    platforms: {
      'windows-x86_64': {
        signature: signature.trim(),
        url: `https://github.com/${repo}/releases/download/v${version}/${setupName(version)}`,
      },
    },
  };
}

/** 更新签名私钥：环境变量优先，其次本机的密钥文件。没有时返回 null（只打普通安装包）。 */
function signingKey() {
  if (process.env.TAURI_SIGNING_PRIVATE_KEY) return process.env.TAURI_SIGNING_PRIVATE_KEY;
  return existsSync(KEY_FILE) ? readFileSync(KEY_FILE, 'utf8').trim() : null;
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

const localAppData = () => process.env.LOCALAPPDATA ?? join(homedir(), 'AppData', 'Local');
/** 只为当前用户安装时可能的位置：新版安装包装进 %LOCALAPPDATA%\Programs\SnapMind，旧版在 %LOCALAPPDATA%\SnapMind。 */
const userDirs = () => [join(localAppData(), 'Programs', 'SnapMind'), join(localAppData(), 'SnapMind')];
/** 实际装在哪：有 exe 的那个，都没有就取第一个（报错时显示）。 */
const installDir = () => userDirs().find((d) => existsSync(join(d, 'snapmind.exe'))) ?? userDirs()[0];
const startMenuLink = () =>
  join(process.env.APPDATA ?? '', 'Microsoft', 'Windows', 'Start Menu', 'Programs', 'SnapMind.lnk');

/** 为所有用户安装时的位置（安装包 installMode 是 both，管理员账号默认装在这里）。 */
const machineDir = () => join(process.env.ProgramFiles ?? 'C:\\Program Files', 'SnapMind');

/** 结束已安装版本的进程（只匹配几个安装位置里的 exe，连同 WebView2 子进程），以便覆盖安装。 */
function stopInstalled() {
  const exes = [...userDirs(), machineDir()].map((d) => `'${join(d, 'snapmind.exe')}'`).join(',');
  const ids = ps(
    `Get-Process snapmind -ErrorAction SilentlyContinue | Where-Object { @(${exes}) -contains $_.Path } | ForEach-Object { $_.Id }`,
  );
  for (const id of ids.split(/\s+/).filter(Boolean)) {
    console.log(`  结束正在运行的 SnapMind（PID ${id}）`);
    spawnSync('taskkill', ['/PID', id, '/T', '/F'], { stdio: 'ignore' });
  }
}

function install(setup, version) {
  stopInstalled();
  console.log(`\n▶ 静默安装 ${setup}`);
  // 安装包会请求「最高可用权限」（管理员账号弹 UAC、装进 Program Files）。本机自动安装没人点 UAC，
  // 用 RunAsInvoker 以当前权限运行，并指定只为当前用户安装（见 userDirs）。
  const r = spawnSync(setup, ['/S', '/CURRENTUSER'], { stdio: 'inherit', env: { ...process.env, __COMPAT_LAYER: 'RunAsInvoker' } });
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
  const key = signingKey();
  const pubkey = JSON.parse(readFileSync(TAURI_CONF, 'utf8')).plugins?.updater?.pubkey ?? '';
  if (key && !pubkey.trim()) throw new Error('有更新签名私钥，但 tauri.conf.json 里的 plugins.updater.pubkey 是空的');
  if (key) {
    console.log('找到更新签名私钥：生成签名和 latest.json');
    const env = {
      ...process.env,
      TAURI_SIGNING_PRIVATE_KEY: key,
      TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? '',
    };
    // 用文件传配置覆盖，避开 cmd 对 JSON 引号的转义
    const extra = join(ROOT, 'target', 'updater.conf.json');
    writeFileSync(extra, JSON.stringify({ bundle: { createUpdaterArtifacts: true } }));
    run('pnpm', ['tauri', 'build', '--config', extra], { env });
  } else {
    run('pnpm', ['tauri', 'build']);
  }

  if (!isWin) return console.log('\n非 Windows 平台：打包完成，跳过安装。');

  const nsis = join(ROOT, 'target', 'release', 'bundle', 'nsis');
  const setup = join(nsis, setupName(version));
  if (!existsSync(setup)) throw new Error(`找不到安装包 ${setup}`);
  if (key) {
    const sig = readFileSync(`${setup}.sig`, 'utf8');
    const manifest = updaterManifest({ version, signature: sig, date: new Date() });
    writeFileSync(join(nsis, 'latest.json'), JSON.stringify(manifest, null, 2) + '\n');
    console.log(`  更新清单：${join(nsis, 'latest.json')}`);
  }
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
