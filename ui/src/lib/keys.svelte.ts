// 设置窗口里共用的「哪些账户已保存 Key」（只有有没有，没有 Key 本身）。
// 模型服务页写，备用链编辑器读。
import { providerKeyStatus } from './ipc';

export const keyStatus = $state<Record<string, boolean>>({});

/** 重新查询这些账户的 Key 状态。 */
export async function refreshKeys(ids: string[]): Promise<void> {
  Object.assign(keyStatus, await providerKeyStatus(ids));
}
