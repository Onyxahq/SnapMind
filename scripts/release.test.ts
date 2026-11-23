import { describe, expect, it } from 'vitest';
import { bumpVersion, readVersion, updaterManifest, writeVersion } from './release.mjs';
import { notesFromSubject } from './publish.mjs';

const TOML = `[workspace]
members = ["a"]

[workspace.package]
version = "0.1.4"
edition = "2021"

[workspace.dependencies]
serde = { version = "1" }
`;

describe('release 版本号', () => {
  it('从 [workspace.package] 读取，不会误读依赖的 version', () => {
    expect(readVersion(TOML)).toBe('0.1.4');
  });

  it('patch / minor / major / 指定版本', () => {
    expect(bumpVersion('0.1.4', 'patch')).toBe('0.1.5');
    expect(bumpVersion('0.1.4', 'minor')).toBe('0.2.0');
    expect(bumpVersion('0.1.4', 'major')).toBe('1.0.0');
    expect(bumpVersion('0.1.4', '0.2.1')).toBe('0.2.1');
    expect(() => bumpVersion('0.1.4', 'beta')).toThrow();
  });

  it('同时写回 Cargo.toml 和 package.json，只改 workspace 版本', () => {
    const out = writeVersion(TOML, '{"name":"snapmind","version":"0.1.4"}', '0.1.5');
    expect(readVersion(out.toml)).toBe('0.1.5');
    expect(out.toml).toContain('serde = { version = "1" }');
    expect(JSON.parse(out.pkg).version).toBe('0.1.5');
  });
});

describe('更新清单 latest.json', () => {
  it('指向这个版本在 Releases 上的安装包，签名去掉首尾空白', () => {
    const m = updaterManifest({ version: '1.2.3', signature: 'c2ln\n', notes: '修复', date: new Date(Date.UTC(2026, 10, 23, 1, 2, 3)) });
    expect(m).toEqual({
      version: '1.2.3',
      notes: '修复',
      pub_date: '2026-11-23T01:02:03.000Z',
      platforms: {
        'windows-x86_64': {
          signature: 'c2ln',
          url: 'https://github.com/Onyxahq/SnapMind/releases/download/v1.2.3/SnapMind_1.2.3_x64-setup.exe',
        },
      },
    });
  });
});

describe('发布说明', () => {
  it('去掉 conventional commit 前缀', () => {
    expect(notesFromSubject('feat(M11.3): 自动更新')).toBe('自动更新');
    expect(notesFromSubject('fix: 修复贴图')).toBe('修复贴图');
    expect(notesFromSubject('普通标题')).toBe('普通标题');
  });
});
