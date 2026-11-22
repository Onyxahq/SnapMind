// 模型服务的纯逻辑：新服务商的 id、合并获取到的模型列表、标签切换。
import type { FetchedModel, ModelConfig, ModelTag, ProviderConfig, ProviderKind, ProviderMeta } from './ipc';

export const TAGS: { tag: ModelTag; label: string }[] = [
  { tag: 'vision', label: '视觉' },
  { tag: 'text', label: '文本' },
  { tag: 'reasoning', label: '推理' },
  { tag: 'local', label: '本地' },
];

export const FORMAT_LABEL: Record<ProviderMeta['format'], string> = {
  openAi: 'OpenAI 兼容',
  claude: 'Claude 原生',
  gemini: 'Gemini 原生',
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
