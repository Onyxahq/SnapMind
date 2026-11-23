// 模型服务的纯逻辑：新服务商的 id、合并获取到的模型列表、标签切换。
import type { FetchedModel, ModelConfig, ModelTag, MtKind, ProviderConfig, ProviderKind, ProviderMeta } from './ipc';
import { msg } from './i18n.svelte';

export interface MtService {
  kind: MtKind;
  name: string;
  mono: string;
  color: string;
  desc: string;
  keyHint: string;
}

/** 机器翻译服务（固定三家，不用添加；填了 Key 就能进备用链）。 */
export const MT_SERVICES: MtService[] = [
  { kind: 'deepl', name: 'DeepL', mono: 'DL', color: '#0F2B46', desc: msg('欧洲语言质量好；免费版每月 50 万字符'), keyHint: msg('免费版 Key 以 :fx 结尾，会自动用免费接口') },
  { kind: 'google', name: msg('Google 翻译'), mono: 'GT', color: '#4285F4', desc: msg('Google Cloud Translation，支持的语言最多'), keyHint: msg('在 Google Cloud 控制台创建的 API Key') },
  { kind: 'microsoft', name: msg('微软翻译'), mono: 'MS', color: '#0078D4', desc: msg('Azure Translator；免费层每月 200 万字符'), keyHint: msg('Azure 翻译资源的「密钥 1」或「密钥 2」') },
];

/** 机器翻译 Key 在系统凭据库里的账户名（与 Rust 端 MtKind::secret_account 一致）。 */
export const mtAccount = (kind: MtKind): string => `mt-${kind}`;

export const TAGS: { tag: ModelTag; label: string }[] = [
  { tag: 'vision', label: msg('视觉') },
  { tag: 'text', label: msg('文本') },
  { tag: 'reasoning', label: msg('推理') },
  { tag: 'local', label: msg('本地') },
];

export const FORMAT_LABEL: Record<ProviderMeta['format'], string> = {
  openAi: msg('OpenAI 兼容'),
  claude: msg('Claude 原生'),
  gemini: msg('Gemini 原生'),
};

/** 新服务商的 id：内置服务商用种类名；自定义服务可以加多个，依次 custom、custom-2…… */
export function newProviderId(kind: ProviderKind, existing: ProviderConfig[]): string {
  const ids = new Set(existing.map((p) => p.id));
  if (!ids.has(kind)) return kind;
  for (let i = 2; ; i++) if (!ids.has(`${kind}-${i}`)) return `${kind}-${i}`;
}

/** 内置服务商（自定义除外）只能添加一次。 */
export function canAdd(kind: ProviderKind, existing: ProviderConfig[]): boolean {
  return kind === 'custom' || !existing.some((p) => p.kind === kind);
}

/**
 * 把新获取的模型列表并入已有列表：已有模型保留勾选和用户改过的标签；
 * 新模型用推断的标签、默认不勾选；服务商已不提供但勾选着的模型保留（可能是手动添加的）。
 */
export function mergeModels(current: ModelConfig[], fetched: FetchedModel[]): ModelConfig[] {
  const old = new Map(current.map((m) => [m.id, m]));
  const seen = new Set<string>();
  const out: ModelConfig[] = [];
  for (const f of fetched) {
    if (seen.has(f.id)) continue;
    seen.add(f.id);
    out.push(old.get(f.id) ?? { id: f.id, tags: f.tags, enabled: false });
  }
  const kept = current.filter((m) => !seen.has(m.id) && m.enabled);
  return [...kept, ...out];
}

/** 勾选的排前面，其余保持原顺序；可选按名字过滤（不区分大小写）。 */
export function visibleModels(models: ModelConfig[], filter = ''): ModelConfig[] {
  const q = filter.trim().toLowerCase();
  const list = q ? models.filter((m) => m.id.toLowerCase().includes(q)) : models;
  return [...list.filter((m) => m.enabled), ...list.filter((m) => !m.enabled)];
}

const ORDER: ModelTag[] = ['vision', 'text', 'reasoning', 'local'];

/** 切换一个标签；「视觉」和「文本」互斥且必有其一（视觉模型本来就能处理文本）。 */
export function toggleTag(tags: ModelTag[], tag: ModelTag): ModelTag[] {
  let next: ModelTag[];
  if (tags.includes(tag)) {
    next = tags.filter((t) => t !== tag);
    if (tag === 'vision') next.push('text');
    if (tag === 'text') next.push('vision');
  } else {
    next = [...tags.filter((t) => !(tag === 'vision' && t === 'text') && !(tag === 'text' && t === 'vision')), tag];
  }
  return [...new Set(next)].sort((a, b) => ORDER.indexOf(a) - ORDER.indexOf(b));
}

/** 国内站还是国际站（只用来给「改用…」按钮起名字）。 */
export function regionOf(url: string): '国内站' | '国际站' {
  const host = url.replace(/^\w+:\/\//, '').split(/[/:]/)[0] ?? '';
  const cn = /\.cn$/.test(host) || ['dashscope.aliyuncs.com', 'api.minimaxi.com', 'api.stepfun.com'].includes(host);
  return cn ? '国内站' : '国际站';
}
