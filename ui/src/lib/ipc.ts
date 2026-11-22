// 前端调用 Rust 命令的唯一入口。命令名、事件名和数据类型集中在这里。
// 在普通浏览器里打开开发页面时（没有 Tauri），用内存里的假实现代替，方便调界面。
import { invoke } from '@tauri-apps/api/core';
import { emit, listen } from '@tauri-apps/api/event';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
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
  capture: { colorFormat: ColorFormat; showHints: boolean; confirmDiscard: boolean };
  save: SaveConfig;
  providers: ProviderConfig[];
  mt: { microsoftRegion: string };
  chains: ChainsConfig;
  /** 空字符串表示用默认提示词 */
  prompts: Record<PromptKind, string>;
  translation: TranslationConfig;
}

export interface TranslationConfig {
  targetLang: string;
  secondLang: string;
  /** 原文已是目标语言时：second 翻成第二语言，skip 不翻译 */
  sameLang: 'second' | 'skip';
  bilingual: boolean;
  keepFormat: boolean;
  autoTerms: boolean;
  /** 不翻译列表 */
  glossary: string[];
  oneshot: boolean;
  /** 截图翻译用原位覆盖（否则用卡片） */
  overlay: boolean;
}

export type PromptKind = 'recognize' | 'translate' | 'oneshot' | 'markdown';

/** 备用链里的一项（与 snapmind-core::config::BackendRef 对应）。 */
export type BackendRef =
  | { type: 'llm'; provider: string; model: string }
  | { type: 'mt'; kind: MtKind }
  | { type: 'localOcr' }
  | { type: 'degrade' };

export type ChainTask = 'translate' | 'recognize' | 'oneshot';

export interface ChainsConfig {
  fallback: boolean;
  timeoutSecs: number;
  translate: BackendRef[];
  recognize: BackendRef[];
  oneshot: BackendRef[];
}

export type MtKind = 'deepl' | 'google' | 'microsoft';

export type ProviderKind = 'deepseek' | 'anthropic' | 'gemini' | 'openai' | 'openrouter' | 'ollama' | 'custom';
export type ModelTag = 'vision' | 'text' | 'reasoning' | 'local';

export interface ModelConfig {
  id: string;
  tags: ModelTag[];
  enabled: boolean;
}

/** 已添加的服务商；API Key 不在这里，存在系统凭据库（账户名 = id）。 */
export interface ProviderConfig {
  id: string;
  kind: ProviderKind;
  name: string;
  baseUrl: string;
  models: ModelConfig[];
}

export type ImageFormat = 'png' | 'jpg' | 'webp';

export interface SaveConfig {
  /** 空字符串表示「图片\SnapMind」 */
  dir: string;
  format: ImageFormat;
  quality: number;
  filenameTemplate: string;
  ctrlS: 'window' | 'system' | 'direct';
  extraDirs: string[];
}

export type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K] };

export const DEFAULT_CONFIG: Config = {
  version: 1,
  appearance: { theme: 'system', accent: 'blue' },
  capture: { colorFormat: 'hex', showHints: true, confirmDiscard: true },
  save: {
    dir: '',
    format: 'png',
    quality: 90,
    filenameTemplate: 'SnapMind_{yyyyMMdd_HHmmss}',
    ctrlS: 'window',
    extraDirs: [],
  },
  providers: [],
  mt: { microsoftRegion: '' },
  chains: { fallback: true, timeoutSecs: 8, translate: [], recognize: [{ type: 'localOcr' }], oneshot: [] },
  prompts: { recognize: '', translate: '', oneshot: '', markdown: '' },
  translation: {
    targetLang: 'zh-Hans',
    secondLang: 'en',
    sameLang: 'second',
    bilingual: true,
    keepFormat: true,
    autoTerms: true,
    glossary: [],
    oneshot: false,
    overlay: false,
  },
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
  /** 选好区域后要直接做什么：capture 只截图；translate 是 Ctrl+Shift+O（截图 → 识别 → 翻译） */
  intent?: 'capture' | 'translate' | 'recognize';
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

// ---- 交出截图：复制、保存 ----

/** 选区，本屏物理像素 */
export interface Region {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface Output {
  format: ImageFormat;
  quality: number;
  width: number;
  height: number;
}

/** 保存目标：path = 系统对话框选的完整路径；dir + name = 保存窗口填的（重名自动加序号）；都空 = 默认目录 + 模板。 */
export interface Target {
  path?: string;
  dir?: string;
  name?: string;
}

export interface SaveInfo {
  dir: string;
  name: string;
  quick: { label: string; path: string }[];
}

const intRegion = (r: Region): Region => ({
  x: Math.round(r.x),
  y: Math.round(r.y),
  width: Math.round(r.width),
  height: Math.round(r.height),
});

/** 复制截图（位图 + PNG + 文件）并结束截图。返回临时文件名。 */
export async function captureCopy(index: number, region: Region): Promise<string> {
  if (!inTauri()) return `SnapMind_dev.png（${region.width}×${region.height}）`;
  return invoke('capture_copy', { index, region: intRegion(region) });
}

/** 保存截图并结束截图。返回保存的完整路径。 */
export async function captureSave(index: number, region: Region, output: Output, target: Target): Promise<string> {
  if (!inTauri()) return target.path ?? `${target.dir ?? 'C:\\Pictures\\SnapMind'}\\${target.name ?? 'SnapMind'}.${output.format}`;
  return invoke('capture_save', { index, region: intRegion(region), output, target });
}

/** 估算保存后的文件大小（真实编码一次）。 */
export async function captureEstimate(index: number, region: Region, output: Output): Promise<number> {
  if (!inTauri()) {
    const bpp = output.format === 'png' ? 1.6 : (0.05 + (output.quality / 100) * 0.9) * (output.format === 'webp' ? 0.7 : 1);
    return Math.round(output.width * output.height * bpp);
  }
  return invoke('capture_estimate', { index, region: intRegion(region), output });
}

export async function saveInfo(): Promise<SaveInfo> {
  if (!inTauri()) {
    return {
      dir: 'C:\\Users\\me\\Pictures\\SnapMind',
      name: 'SnapMind_20261122_101530',
      quick: [
        { label: '桌面', path: 'C:\\Users\\me\\Desktop' },
        { label: '图片', path: 'C:\\Users\\me\\Pictures' },
        { label: 'SnapMind', path: 'C:\\Users\\me\\Pictures\\SnapMind' },
      ],
    };
  }
  return invoke('save_info');
}

export async function pickSaveFile(dir: string, name: string, format: ImageFormat): Promise<string | null> {
  if (!inTauri()) return null;
  return invoke('pick_save_file', { dir, name, format });
}

export async function pickFolder(dir: string): Promise<string | null> {
  if (!inTauri()) return null;
  return invoke('pick_folder', { dir });
}

export async function openFolder(path: string): Promise<void> {
  if (!inTauri()) return;
  return invoke('open_folder', { path });
}

/** 上传标注层（相对选区的偏移 + RGBA 像素；传 null 清除）。用二进制请求体，不经过 JSON。 */
export async function captureSetAnnotations(
  index: number,
  layer: { x: number; y: number; width: number; height: number; rgba: Uint8Array } | null,
): Promise<void> {
  if (!inTauri()) return;
  const h = (v: number | undefined) => String(v ?? 0);
  return invoke('capture_set_annotations', layer?.rgba ?? new Uint8Array(0), {
    headers: {
      'x-index': String(index),
      'x-x': h(layer?.x),
      'x-y': h(layer?.y),
      'x-width': h(layer?.width),
      'x-height': h(layer?.height),
    },
  });
}

// ---- 贴图 ----

/** 把截图钉在原位置（带标注），并结束截图。返回贴图 id。 */
export async function capturePin(index: number, region: Region): Promise<number> {
  if (!inTauri()) return 0;
  return invoke('capture_pin', { index, region: intRegion(region) });
}

export async function pinInfo(): Promise<{ width: number; height: number; text: string | null } | null> {
  if (!inTauri()) return { width: 400, height: 300, text: null };
  return invoke('pin_info');
}

export async function pinReady(): Promise<void> {
  if (!inTauri()) return;
  return invoke('pin_ready');
}

/** 缩放贴图：factor 相对倍数，absolute 目标比例；x、y 为窗口内的锚点（CSS 像素）。 */
export async function pinZoom(a: { factor?: number; absolute?: number; x: number; y: number }): Promise<number | null> {
  if (!inTauri()) return null;
  return invoke('pin_zoom', { factor: a.factor ?? null, absolute: a.absolute ?? null, x: a.x, y: a.y });
}

export async function pinClose(): Promise<void> {
  if (!inTauri()) return;
  return invoke('pin_close');
}

/** 鼠标穿透状态变化（Ctrl+Shift+X 由 Rust 全局处理）。 */
export async function onPinThrough(cb: (on: boolean) => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  return listen<boolean>('pin:through', (e) => cb(e.payload));
}

/** 让当前贴图鼠标穿透（之后用全局 Ctrl+Shift+X 或托盘菜单取消）。 */
export async function pinThrough(): Promise<void> {
  if (!inTauri()) return;
  return invoke('pin_through');
}

// ---- 贴图工具条 ----

export type PinAction =
  | 'copy'
  | 'save'
  | 'close'
  | 'through'
  | 'zoom100'
  | 'opacity'
  | 'annotate'
  | 'done'
  | 'tool'
  | 'opts'
  | 'undo';

/** 工具条发给贴图的动作。 */
export interface PinActionMsg {
  action: PinAction;
  value?: number;
  tool?: import('./annotate').Tool;
  opts?: Partial<import('./annotate').ToolOpts>;
}

/** 贴图告诉工具条自己的标注状态。 */
export interface PinAnnotState {
  id: number;
  active: boolean;
  tool: import('./annotate').Tool | null;
  opts: Record<import('./annotate').Tool, import('./annotate').ToolOpts>;
  canUndo: boolean;
}

export async function pinHover(on: boolean, opacity: number): Promise<void> {
  if (!inTauri()) return;
  return invoke('pin_hover', { on, opacity });
}

export async function pinOpacity(opacity: number): Promise<void> {
  if (!inTauri()) return;
  return invoke('pin_opacity', { opacity });
}

export async function pinCopy(id: number): Promise<void> {
  if (!inTauri()) return;
  return invoke('pin_copy', { id });
}

/** 系统另存为保存贴图，返回保存路径（取消时为 null）。 */
export async function pinSave(id: number): Promise<string | null> {
  if (!inTauri()) return null;
  return invoke('pin_save', { id });
}

/** 文字卡片排版好后把 RGBA 交给 Rust。 */
export async function pinSetImage(width: number, height: number, rgba: Uint8Array): Promise<void> {
  if (!inTauri()) return;
  return invoke('pin_set_image', rgba, { headers: { 'x-width': String(width), 'x-height': String(height) } });
}

export async function toolsHover(on: boolean): Promise<void> {
  if (!inTauri()) return;
  return invoke('tools_hover', { on });
}

export async function toolsMenu(open: boolean): Promise<void> {
  if (!inTauri()) return;
  return invoke('tools_menu', { open });
}

export async function toolsLayout(width: number, height: number): Promise<void> {
  if (!inTauri()) return;
  return invoke('tools_layout', { width, height });
}

export interface ToolsTarget {
  id: number;
  placement: 'below' | 'above';
  opacity: number;
}

export async function onToolsTarget(cb: (t: ToolsTarget) => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  return listen<ToolsTarget>('tools:target', (e) => cb(e.payload));
}

/** 工具条把动作发给对应的贴图窗口。 */
export async function sendPinAction(id: number, msg: PinActionMsg): Promise<void> {
  if (!inTauri()) return;
  const { emitTo } = await import('@tauri-apps/api/event');
  return emitTo(`pin-${id}`, 'pin:action', msg);
}

export async function onPinAction(cb: (a: PinActionMsg) => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  return listen<PinActionMsg>('pin:action', (e) => cb(e.payload));
}

/** 贴图把标注状态发给工具条。 */
export async function sendAnnotState(state: PinAnnotState): Promise<void> {
  if (!inTauri()) return;
  const { emitTo } = await import('@tauri-apps/api/event');
  return emitTo('pintools', 'pin:annot', state);
}

export async function onAnnotState(cb: (s: PinAnnotState) => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  return listen<PinAnnotState>('pin:annot', (e) => cb(e.payload));
}

/** 贴图进入 / 退出标注模式（工具条保持显示）。 */
export async function pinKeep(on: boolean): Promise<void> {
  if (!inTauri()) return;
  return invoke('pin_keep', { on });
}

/** 上传贴图的标注层（相对图片的偏移 + RGBA；null 清除）。 */
export async function pinSetAnnotations(
  layer: { x: number; y: number; width: number; height: number; rgba: Uint8Array } | null,
): Promise<void> {
  if (!inTauri()) return;
  const h = (v: number | undefined) => String(v ?? 0);
  return invoke('pin_set_annotations', layer?.rgba ?? new Uint8Array(0), {
    headers: { 'x-x': h(layer?.x), 'x-y': h(layer?.y), 'x-width': h(layer?.width), 'x-height': h(layer?.height) },
  });
}

// ---- 历史记录 ----

export interface PinRecord {
  id: number;
  createdAt: number;
  closedAt: number;
  width: number;
  height: number;
  imagePath: string;
  thumbPath: string;
  text: string;
  source: 'capture' | 'clipboard' | 'text';
  bytes: number;
}

/** 贴图历史（最近关闭的在前），query 按识别文字搜索。 */
export async function historyPins(query?: string, limit?: number): Promise<PinRecord[]> {
  if (!inTauri()) return [];
  return invoke('history_pins', { query: query ?? null, limit: limit ?? null });
}

// ---- 模型服务 ----

export interface ProviderMeta {
  kind: ProviderKind;
  name: string;
  mono: string;
  color: string;
  format: 'openAi' | 'claude' | 'gemini';
  defaultUrl: string;
  needsKey: boolean;
  desc: string;
}

export interface FetchedModel {
  id: string;
  tags: ModelTag[];
}

// 浏览器里调界面用的假数据（真实目录在 Rust 的 snapmind-providers::catalog）
const FAKE_CATALOG: ProviderMeta[] = [
  { kind: 'deepseek', name: 'DeepSeek', mono: 'DS', color: '#4D6BFE', format: 'openAi', defaultUrl: 'https://api.deepseek.com/v1', needsKey: true, desc: '性价比高的中文文本模型' },
  { kind: 'anthropic', name: 'Anthropic', mono: 'AN', color: '#C2410C', format: 'claude', defaultUrl: 'https://api.anthropic.com', needsKey: true, desc: 'Claude 系列，支持图片输入' },
  { kind: 'ollama', name: 'Ollama（本地）', mono: 'OL', color: '#334155', format: 'openAi', defaultUrl: 'http://localhost:11434/v1', needsKey: false, desc: '在本机运行模型，不需要 Key' },
  { kind: 'custom', name: '自定义服务', mono: '+', color: '#64748B', format: 'openAi', defaultUrl: 'https://', needsKey: true, desc: '任意 OpenAI 兼容接口' },
];
const fakeKeys = new Set<string>();

export async function providerCatalog(): Promise<ProviderMeta[]> {
  if (!inTauri()) return FAKE_CATALOG;
  return invoke('provider_catalog');
}

/** 哪些服务商已保存 Key（不回传 Key 本身）。 */
export async function providerKeyStatus(ids: string[]): Promise<Record<string, boolean>> {
  if (!inTauri()) return Object.fromEntries(ids.map((id) => [id, fakeKeys.has(id)]));
  return invoke('provider_key_status', { ids });
}

/** 保存 Key 到系统凭据库；空字符串表示删除。 */
export async function providerSetKey(id: string, key: string): Promise<void> {
  if (!inTauri()) {
    if (key.trim()) fakeKeys.add(id);
    else fakeKeys.delete(id);
    return;
  }
  return invoke('provider_set_key', { id, key });
}

/** 获取模型列表（同时就是测试连接）；失败时抛出可直接展示的中文错误。 */
export async function providerFetchModels(p: Pick<ProviderConfig, 'id' | 'kind' | 'baseUrl'>): Promise<FetchedModel[]> {
  if (!inTauri()) {
    await new Promise((r) => setTimeout(r, 500));
    if (p.kind !== 'ollama' && !fakeKeys.has(p.id)) throw '请先填写 API Key';
    return [
      { id: `${p.kind}-chat`, tags: ['text'] },
      { id: `${p.kind}-reasoner`, tags: ['text', 'reasoning'] },
      { id: `${p.kind}-vl`, tags: ['vision'] },
    ];
  }
  return invoke('provider_fetch_models', { id: p.id, kind: p.kind, baseUrl: p.baseUrl });
}

/** 测试机器翻译（用已保存的 Key 把一句英文翻成简体中文），返回译文。 */
export async function mtTest(kind: MtKind): Promise<string> {
  if (!inTauri()) {
    await new Promise((r) => setTimeout(r, 400));
    if (!fakeKeys.has(`mt-${kind}`)) throw '请先填写 API Key';
    return '你好，世界！';
  }
  return invoke('mt_test', { kind });
}

// ---- 提示词与语言 ----

/** 四套默认提示词（以 Rust 端 snapmind-core::prompts 为准）。 */
export async function promptDefaults(): Promise<Record<PromptKind, string>> {
  if (!inTauri()) {
    return {
      recognize: '（开发页）识别图片中的全部文字，按阅读顺序输出。',
      translate: '（开发页）把下面的文字翻译成 {target_lang}。\n- 以下术语保持原样：{glossary}\n- 只输出译文\n\n{text}',
      oneshot: '（开发页）识别图片中的全部文字，并翻译成 {target_lang}。',
      markdown: '（开发页）把图片内容整理成 Markdown。',
    };
  }
  return invoke('prompt_defaults');
}

export interface Lang {
  code: string;
  name: string;
}

export async function languages(): Promise<Lang[]> {
  if (!inTauri()) return [{ code: 'zh-Hans', name: '简体中文' }, { code: 'en', name: 'English' }, { code: 'ja', name: '日语' }];
  return invoke('languages');
}

// ---- 识别 / 翻译任务 ----

/** Rust 推来的任务事件（见 src-tauri/src/translate.rs）。 */
export interface TaskEvent {
  id: string;
  kind: 'stage' | 'src' | 'src-reset' | 'dst' | 'dst-reset' | 'done' | 'error' | 'overlay';
  stage: 'recognize' | 'oneshot' | 'translate' | 'markdown';
  text: string;
}

const fakeTaskListeners = new Set<(e: TaskEvent) => void>();
const fakeTimers = new Map<string, ReturnType<typeof setTimeout>[]>();

/** 浏览器开发页里模拟一次流式输出（含一次切换后端）。 */
function fakeRun(id: string, steps: Omit<TaskEvent, 'id'>[]) {
  fakeTimers.get(id)?.forEach(clearTimeout);
  fakeTimers.set(
    id,
    steps.map((s, i) => setTimeout(() => fakeTaskListeners.forEach((f) => f({ id, ...s })), 120 * (i + 1))),
  );
}

const fakeTranslate = (text: string): Omit<TaskEvent, 'id'>[] => [
  { kind: 'stage', stage: 'translate', text: 'deepseek-chat' },
  { kind: 'dst', stage: 'translate', text: '半截' },
  { kind: 'dst-reset', stage: 'translate', text: '' },
  { kind: 'stage', stage: 'translate', text: 'DeepL' },
  ...`（译文）${text}`.match(/.{1,6}/gs)!.map((t) => ({ kind: 'dst' as const, stage: 'translate' as const, text: t })),
  { kind: 'done', stage: 'translate', text: 'deepseek-chat 超时，已切换到 DeepL · 1.4 s' },
];

export async function onTaskEvent(cb: (e: TaskEvent) => void): Promise<() => void> {
  if (!inTauri()) {
    fakeTaskListeners.add(cb);
    return () => fakeTaskListeners.delete(cb);
  }
  return listen<TaskEvent>('task:event', (e) => cb(e.payload));
}

/** 截图翻译：识别选区（原图，不带标注）再翻译，进度通过 onTaskEvent 推来。 */
export async function captureTranslate(id: string, index: number, region: Region): Promise<void> {
  if (!inTauri()) {
    const src = 'Translation is not just about swapping words.\nA good translation keeps intent and tone.';
    return fakeRun(id, [
      { kind: 'stage', stage: 'recognize', text: '' },
      { kind: 'stage', stage: 'recognize', text: 'qwen-vl' },
      ...src.match(/.{1,12}/gs)!.map((t) => ({ kind: 'src' as const, stage: 'recognize' as const, text: t })),
      ...fakeTranslate(src),
    ]);
  }
  return invoke('capture_translate', { id, index, region });
}

/** 截图翻译 · 原位覆盖：本地 OCR → 按段落翻译，段落块通过 overlay 事件推来。 */
export async function captureTranslateOverlay(id: string, index: number, region: Region): Promise<void> {
  if (!inTauri()) {
    const paras = [
      ['Screen text is easy to read back.', '屏幕上的文字很容易读回来。'],
      ['Translations are drawn in place,', '译文直接画在原文的位置上，'],
      ['42.0 %', ''],
    ];
    const h = Math.max(16, Math.min(28, Math.round(region.height / 10)));
    const blocks = paras.map(([src, dst], i) => ({
      x: h,
      y: h + i * h * 2.2,
      width: Math.min(region.width - 2 * h, Math.round(src.length * h * 0.52)),
      height: Math.round(h * 1.5),
      fg: [30, 41, 59],
      bg: [248, 250, 252],
      size: h,
      keep: !dst,
      src,
      dst,
    }));
    const src = paras.filter((p) => p[1]).map((p) => p[0]).join('\n\n');
    const dst = paras.filter((p) => p[1]).map((p) => p[1]).join('\n\n');
    return fakeRun(id, [
      { kind: 'stage', stage: 'recognize', text: '本地 OCR' },
      { kind: 'src', stage: 'recognize', text: src },
      { kind: 'stage', stage: 'translate', text: 'DeepL' },
      { kind: 'dst', stage: 'translate', text: dst },
      { kind: 'overlay', stage: 'translate', text: JSON.stringify(blocks) },
      { kind: 'done', stage: 'translate', text: '本地 OCR → DeepL · 0.9 s' },
    ]);
  }
  return invoke('capture_translate_overlay', { id, index, region });
}

/** 识别为 Markdown：按识别链，Markdown 源码通过 onTaskEvent 的 src 事件（stage = markdown）流式推来。 */
export async function captureMarkdown(id: string, index: number, region: Region): Promise<void> {
  if (!inTauri()) {
    const md = '# 识别为 Markdown\n\n截图里的**标题**、列表和代码会保留结构，==重点==也能显示。\n\n- 编辑视图改源码\n- 预览视图实时渲染\n\n| 快捷键 | 动作 |\n|---|---|\n| Shift+Alt+R | Markdown |\n\n> [!tip] 提示\n> 复制或保存后结束截图。\n';
    return fakeRun(id, [
      { kind: 'stage', stage: 'markdown', text: '' },
      { kind: 'stage', stage: 'markdown', text: 'qwen-vl' },
      ...md.match(/[\s\S]{1,24}/g)!.map((t) => ({ kind: 'src' as const, stage: 'markdown' as const, text: t })),
      { kind: 'done', stage: 'markdown', text: 'qwen-vl · 2.1 s' },
    ]);
  }
  return invoke('capture_markdown', { id, index, region });
}

/** 把 Markdown 存成 .md（系统另存为），返回保存路径，取消时为 null。 */
export async function saveMarkdown(text: string): Promise<string | null> {
  if (!inTauri()) return 'C:\\Users\\demo\\Pictures\\SnapMind\\demo.md';
  return invoke('save_markdown', { text });
}

/**
 * 按（改过的）原文重新翻译。`target` 这次翻成哪种语言（不给就按设置），
 * `selection` 表示来自划词（翻译历史里记成划词）。
 */
export async function translateText(
  id: string,
  text: string,
  opts: { target?: string; selection?: boolean } = {},
): Promise<void> {
  if (!inTauri()) return fakeRun(id, fakeTranslate(text));
  return invoke('translate_text', { id, text, target: opts.target ?? null, selection: opts.selection ?? null });
}

// ---- 划词卡片（见 src-tauri/src/selcard.rs）----

export interface SelcardShow {
  text: string;
  /** automation（UI Automation）/ copy（模拟复制） */
  method: string;
}

/** Rust 把卡片摆到鼠标旁、显示出来时通知页面（文字用 selcardPending 去取）。 */
export async function onSelcardShow(cb: () => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  // 只收发给本窗口的（同时钉着好几张卡片时，别的卡片的通知不能把这张重置掉）
  return getCurrentWebviewWindow().listen('selcard:show', () => cb());
}

let fakeSelcardShown = false;
/** 取走等着翻译的文字（只取一次；新建的卡片页面加载完时也来取，补上加载前发出的通知）。 */
export async function selcardPending(): Promise<SelcardShow | null> {
  if (!inTauri()) {
    if (fakeSelcardShown) return null;
    fakeSelcardShown = true;
    return { text: 'Selection translation shows the result right next to the mouse pointer.', method: 'automation' };
  }
  return invoke('selcard_pending');
}

/** 卡片被收起（点了别处、Esc、关闭）。 */
export async function onSelcardHide(cb: () => void): Promise<() => void> {
  if (!inTauri()) return () => {};
  return getCurrentWebviewWindow().listen('selcard:hide', () => cb());
}

/** 报告卡片内容高度（CSS 像素），Rust 调整窗口大小和位置。 */
export async function selcardResize(height: number): Promise<void> {
  if (!inTauri()) return;
  return invoke('selcard_resize', { height });
}

export async function selcardPin(on: boolean): Promise<void> {
  if (!inTauri()) return;
  return invoke('selcard_pin', { on });
}

export async function selcardClose(): Promise<void> {
  if (!inTauri()) return;
  return invoke('selcard_close');
}

export async function taskCancel(id: string): Promise<void> {
  if (!inTauri()) {
    fakeTimers.get(id)?.forEach(clearTimeout);
    fakeTimers.delete(id);
    return;
  }
  return invoke('task_cancel', { id });
}

// ---- 本地 OCR（见 src-tauri/src/ocr.rs）----

/** 识别出的一行；坐标相对选区左上角（截图物理像素）。 */
export interface OcrLine {
  x: number;
  y: number;
  width: number;
  height: number;
  text: string;
}

export interface OcrResult {
  lines: OcrLine[];
  /** 段落：行下标，按阅读顺序 */
  paragraphs: number[][];
  /** 按段落拼好的全文 */
  text: string;
  ms: number;
}

export interface OcrStatus {
  installed: boolean;
  downloading: boolean;
  received: number;
  total: number;
  error: string | null;
}

const OCR_TOTAL = 21_510_548;
let fakeOcr: OcrStatus = { installed: false, downloading: false, received: 0, total: OCR_TOTAL, error: null };
const fakeOcrListeners = new Set<(s: OcrStatus) => void>();
const fakeOcrResults = new Map<string, OcrLine[]>();
const fakeOcrParas = new Map<string, number[][]>();

/** 浏览器开发页：在选区里排几段假文字（两段正文 + 一个列表）。 */
function fakeOcrResult(region: Region): OcrResult {
  const texts = [
    ['Screenshots hold text that was never', 'meant to be copied, like labels in', 'a dialog or a frame of a video.'],
    ['识别后的文字按段落排好，段落内部的', '换行会被去掉，可以直接粘贴。'],
    ['• 右键划过检测框只选一部分'],
    ['• 右键单击恢复全部'],
  ];
  const lh = Math.max(14, Math.min(30, Math.round(region.height / 14)));
  const lines: OcrLine[] = [];
  const paragraphs: number[][] = [];
  let y = Math.round(lh * 0.8);
  for (const p of texts) {
    const ids: number[] = [];
    for (const t of p) {
      if (y + lh > region.height) break;
      const width = Math.min(region.width - 2 * lh, Math.round(t.length * lh * (/[一-鿿]/.test(t) ? 0.9 : 0.48)));
      ids.push(lines.length);
      lines.push({ x: lh, y, width: Math.max(8, width), height: lh, text: t });
      y += Math.round(lh * 1.45);
    }
    if (ids.length) paragraphs.push(ids);
    y += Math.round(lh * 0.9);
  }
  const join = (ids: number[]) => ids.map((i) => lines[i].text).join(' ');
  return { lines, paragraphs, text: paragraphs.map(join).join('\n\n'), ms: 182 };
}

/** 识别截图选区（原图，不带标注）。模型没下载时报错，先用 ocrStatus 判断。 */
export async function captureOcr(id: string, index: number, region: Region): Promise<OcrResult> {
  if (!inTauri()) {
    await new Promise((r) => setTimeout(r, 500));
    const r = fakeOcrResult(region);
    fakeOcrResults.set(id, r.lines);
    fakeOcrParas.set(id, r.paragraphs);
    return r;
  }
  return invoke('capture_ocr', { id, index, region });
}

/** 按选中的行（null = 全部）重新分段拼接；byLine 时一行一行。 */
export async function ocrCompose(id: string, pick: number[] | null, byLine: boolean): Promise<string> {
  if (!inTauri()) {
    const lines = fakeOcrResults.get(id) ?? [];
    const keep = new Set(pick ?? lines.map((_, i) => i));
    const paras = (fakeOcrParas.get(id) ?? []).map((p) => p.filter((i) => keep.has(i))).filter((p) => p.length);
    return byLine
      ? paras.flat().map((i) => lines[i].text).join('\n')
      : paras.map((p) => p.map((i) => lines[i].text).join(' ')).join('\n\n');
  }
  return invoke('ocr_compose', { id, pick, byLine });
}

export async function ocrForget(id: string): Promise<void> {
  if (!inTauri()) {
    fakeOcrResults.delete(id);
    return;
  }
  return invoke('ocr_forget', { id });
}

export async function ocrStatus(): Promise<OcrStatus> {
  if (!inTauri()) return fakeOcr;
  return invoke('ocr_status');
}

/** 下载 OCR 模型；进度通过 onOcrDownload 推来。 */
export async function ocrDownload(): Promise<void> {
  if (!inTauri()) {
    if (fakeOcr.downloading) return;
    fakeOcr = { ...fakeOcr, downloading: true, received: 0, error: null };
    for (let i = 1; i <= 20; i++) {
      await new Promise((r) => setTimeout(r, 90));
      fakeOcr = { ...fakeOcr, received: Math.round((OCR_TOTAL * i) / 20) };
      fakeOcrListeners.forEach((f) => f(fakeOcr));
    }
    fakeOcr = { ...fakeOcr, downloading: false, installed: true };
    fakeOcrListeners.forEach((f) => f(fakeOcr));
    return;
  }
  return invoke('ocr_download');
}

export async function onOcrDownload(cb: (s: OcrStatus) => void): Promise<() => void> {
  if (!inTauri()) {
    fakeOcrListeners.add(cb);
    return () => fakeOcrListeners.delete(cb);
  }
  return listen<OcrStatus>('ocr:download', (e) => cb(e.payload));
}

/** 贴图的自动识别结果：行的笔画范围（图片像素）和段落。 */
export interface PinOcr {
  lines: OcrLine[];
  paragraphs: number[][];
}

/** 贴图贴出后自动识别（文字卡片、没下载模型时为 null）。 */
export async function pinOcr(): Promise<PinOcr | null> {
  if (!inTauri()) {
    // 浏览器开发页：在 400×300 的假贴图上排几行
    const texts = ['Pinned screenshots can be read back.', 'Drag across the words to select them,', 'then press Ctrl+C to copy.', '中文也可以直接拖选复制。'];
    const lines = texts.map((text, i) => ({ x: 20, y: 30 + i * 34 + (i === 3 ? 30 : 0), width: Math.round(text.length * (i === 3 ? 15 : 8.2)), height: 15, text }));
    return { lines, paragraphs: [[0, 1, 2], [3]] };
  }
  return invoke('pin_ocr');
}

/** 复制贴图上拖选的文字：[行号, 选中的部分]，Rust 按段落规则拼接后写进剪贴板。 */
export async function pinCopyText(pieces: [number, string][]): Promise<string> {
  if (!inTauri()) return pieces.map((p) => p[1]).join(' ');
  return invoke('pin_copy_text', { pieces });
}

/** 把文字贴成文字卡片（放在选区左上角）并结束截图。 */
export async function capturePinText(index: number, region: Region, text: string): Promise<number> {
  if (!inTauri()) return 0;
  return invoke('capture_pin_text', { index, region, text });
}

export interface TransRecord {
  id: number;
  createdAt: number;
  updatedAt: number;
  source: string;
  translation: string;
  sourceLang: string;
  targetLang: string;
  /** 实际使用的后端和切换说明 */
  backend: string;
  origin: 'capture' | 'selection';
}

/** 翻译历史（最近的在前），query 在原文和译文里搜索。 */
export async function historyTranslations(query?: string, limit?: number): Promise<TransRecord[]> {
  if (!inTauri()) return [];
  return invoke('history_translations', { query: query ?? null, limit: limit ?? null });
}

export async function historyDeleteTranslation(id: number): Promise<boolean> {
  if (!inTauri()) return false;
  return invoke('history_delete_translation', { id });
}
