// 每个窗口共用的响应式配置。窗口启动时调用 startConfig()，之后读 config.value 即可，
// 任何窗口改了配置，所有窗口都会收到 config-changed 并立即刷新（含主题）。
import {
  DEFAULT_CONFIG,
  configGet,
  configUpdate,
  onConfigChanged,
  systemLanguage,
  type Config,
  type DeepPartial,
} from './ipc';
import { resolveLanguage, setLocale } from './i18n.svelte';
import { applyMotion, applyTheme } from './theme';

export const config = $state<{ value: Config }>({ value: structuredClone(DEFAULT_CONFIG) });

let sysTag = '';

/** 系统首选的界面语言标签（启动时读一次）。 */
export const systemTag = (): string => sysTag;

function set(c: Config): Promise<void> {
  config.value = c;
  applyTheme(c.appearance.theme, c.appearance.accent);
  applyMotion(c.appearance.motion);
  return setLocale(resolveLanguage(c.general.language, sysTag));
}

let started = false;

/** 读取配置、应用主题、订阅变更。重复调用无副作用。 */
export async function startConfig(): Promise<void> {
  if (started) return;
  started = true;
  sysTag = await systemLanguage().catch(() => '');
  await onConfigChanged((c) => void set(c));
  // 语言词典加载完再返回，窗口挂载时就是正确的语言
  await set(await configGet());
}

/** 修改配置。返回值即为生效后的完整配置。 */
export async function updateConfig(patch: DeepPartial<Config>): Promise<Config> {
  const next = await configUpdate(patch);
  await set(next);
  return next;
}
