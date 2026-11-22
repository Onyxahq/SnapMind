// 每个窗口共用的响应式配置。窗口启动时调用 startConfig()，之后读 config.value 即可，
// 任何窗口改了配置，所有窗口都会收到 config-changed 并立即刷新（含主题）。
import {
  DEFAULT_CONFIG,
  configGet,
  configUpdate,
  onConfigChanged,
  type Config,
  type DeepPartial,
} from './ipc';
import { applyMotion, applyTheme } from './theme';

export const config = $state<{ value: Config }>({ value: structuredClone(DEFAULT_CONFIG) });

function set(c: Config) {
  config.value = c;
  applyTheme(c.appearance.theme, c.appearance.accent);
  applyMotion(c.appearance.motion);
}

let started = false;

/** 读取配置、应用主题、订阅变更。重复调用无副作用。 */
export async function startConfig(): Promise<void> {
  if (started) return;
  started = true;
  await onConfigChanged(set);
  set(await configGet());
}

/** 修改配置。返回值即为生效后的完整配置。 */
export async function updateConfig(patch: DeepPartial<Config>): Promise<Config> {
  const next = await configUpdate(patch);
  set(next);
  return next;
}
