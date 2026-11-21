// 前端调用 Rust 命令的唯一入口。命令名、事件名和数据类型集中在这里。
// 在普通浏览器里打开开发页面时（没有 Tauri），用内存里的假实现代替，方便调界面。
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import type { Accent, ThemeMode } from './theme';
import type { ColorFormat } from './color';

export interface AppInfo {
  name: string;
  version: string;
  platform: string;
}

/** 与 snapmind-core::config::Config 一一对应（camelCase）。 */
export interface Config {
  version: number;
  appearance: { theme: ThemeMode; accent: Accent };
  capture: { colorFormat: ColorFormat; showHints: boolean };
}

export type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K] };

export const DEFAULT_CONFIG: Config = {
  version: 1,
  appearance: { theme: 'system', accent: 'blue' },
  capture: { colorFormat: 'hex', showHints: true },
};

export const CONFIG_CHANGED = 'config-changed';

/** 是否运行在 Tauri 窗口里（纯浏览器打开 dev 页面时为 false）。 */
export const inTauri = (): boolean =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

/** JSON Merge Patch（RFC 7386），只用于浏览器里的假实现；真实合并在 Rust 里做。 */
export function mergePatch<T>(target: T, patch: unknown): T {
  if (patch === null || typeof patch !== 'object' || Array.isArray(patch)) return patch as T;
  const out: Record<string, unknown> =
    target && typeof target === 'object' && !Array.isArray(target) ? { ...(target as object) } : {};
  for (const [k, v] of Object.entries(patch)) {
    if (v === null) delete out[k];
    else out[k] = mergePatch(out[k], v);
  }
  return out as T;
}

// ---- 浏览器里的假实现 ----
let fakeConfig: Config = structuredClone(DEFAULT_CONFIG);
const fakeListeners = new Set<(c: Config) => void>();

export async function appInfo(): Promise<AppInfo> {
  if (!inTauri()) return { name: 'SnapMind', version: 'dev', platform: 'browser' };
  return invoke<AppInfo>('app_info');
}

export async function configGet(): Promise<Config> {
  if (!inTauri()) return structuredClone(fakeConfig);
  return invoke<Config>('config_get');
}

/** 提交配置补丁；非法值会被 Rust 端拒绝并抛出错误信息。 */
export async function configUpdate(patch: DeepPartial<Config>): Promise<Config> {
  if (!inTauri()) {
    fakeConfig = mergePatch(fakeConfig, patch);
    fakeListeners.forEach((f) => f(structuredClone(fakeConfig)));
    return structuredClone(fakeConfig);
  }
  return invoke<Config>('config_update', { patch });
}

/** 订阅配置变更（任何窗口改了配置都会收到）。返回取消订阅函数。 */
export async function onConfigChanged(cb: (c: Config) => void): Promise<() => void> {
  if (!inTauri()) {
    fakeListeners.add(cb);
    return () => fakeListeners.delete(cb);
  }
  return listen<Config>(CONFIG_CHANGED, (e) => cb(e.payload));
}

// ---- 截图遮罩 ----

export interface CaptureStart {
  seq: number;
  /** 显示器序号，也是 snapframe 协议里的帧路径 */
  index: number;
  monitor: import('./geometry').MonitorGeom;
  hasCursor: boolean;
  /** 截图时的鼠标位置（虚拟桌面物理坐标），放大镜一出现就能定位 */
  cursor: import('./geometry').Point | null;
}

/** 订阅「开始截图」（只有遮罩窗口会收到）。 */
export async function onCaptureStart(cb: (s: CaptureStart) => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  return listen<CaptureStart>('capture:start', (e) => cb(e.payload));
}

/** 回报遮罩进度：mask = 遮罩已出现，frame = 冻结帧已画好。返回距按键的毫秒数。 */
export async function overlayReady(seq: number, stage: 'mask' | 'frame'): Promise<number | null> {
  if (!inTauri()) return null;
  return invoke<number | null>('overlay_ready', { seq, stage, at: Date.now() });
}

export async function overlayEnd(): Promise<void> {
  if (!inTauri()) return;
  return invoke('overlay_end');
}

/** 某个显示器上开始新建选区：通知其他遮罩窗口清掉自己的选区（同一时间只有一个选区）。 */
export async function claimSelection(index: number): Promise<void> {
  if (!inTauri()) return;
  return emit('overlay:claim', index);
}

export async function onSelectionClaimed(cb: (index: number) => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  return listen<number>('overlay:claim', (e) => cb(e.payload));
}

/** 吸附链：物理坐标处从最内层控件到顶层窗口的矩形（虚拟桌面物理坐标），桌面空白处为空。 */
export async function snapChain(x: number, y: number): Promise<import('./geometry').Rect[]> {
  if (!inTauri()) return [];
  return invoke('snap_chain', { x: Math.round(x), y: Math.round(y) });
}

/** 把系统鼠标移动若干物理像素（W A S D 逐像素移动）。 */
export async function cursorNudge(dx: number, dy: number): Promise<void> {
  if (!inTauri()) return;
  return invoke('cursor_nudge', { dx, dy });
}

/** 写入剪贴板文字。走 Rust（系统剪贴板），不依赖 WebView 的剪贴板权限。 */
export async function clipboardWriteText(text: string): Promise<void> {
  if (!inTauri()) return navigator.clipboard.writeText(text);
  return invoke('clipboard_write_text', { text });
}
