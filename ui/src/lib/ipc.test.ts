import { describe, expect, it } from 'vitest';
import { appInfo, inTauri } from './ipc';

describe('ipc', () => {
  it('在非 Tauri 环境下返回占位信息', async () => {
    expect(inTauri()).toBe(false);
    const info = await appInfo();
    expect(info.name).toBe('SnapMind');
    expect(info.platform).toBe('browser');
  });
});
