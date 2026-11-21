import { describe, expect, it } from 'vitest';
import { bumpVersion, readVersion, writeVersion } from './release.mjs';

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
