// 备用链的纯逻辑：每一项的显示名 / 标签 / 状态，可添加的候选，移动与一步到位链的「降级」置底。
import type { BackendRef, ChainTask, MtKind, ProviderConfig } from './ipc';
import { MT_SERVICES, mtAccount } from './providers';

export const TASKS: { value: ChainTask; label: string }[] = [
  { value: 'translate', label: '翻译' },
  { value: 'recognize', label: '识别' },
  { value: 'oneshot', label: '一步到位' },
];

export function refKey(r: BackendRef): string {
  switch (r.type) {
    case 'llm':
      return `llm:${r.provider}/${r.model}`;
    case 'mt':
      return `mt:${r.kind}`;
    default:
      return r.type;
  }
}

const mtName = (k: MtKind) => MT_SERVICES.find((s) => s.kind === k)?.name ?? k;

export function refLabel(r: BackendRef, providers: ProviderConfig[]): string {
  switch (r.type) {
    case 'llm': {
      const p = providers.find((x) => x.id === r.provider);
      return `${p?.name ?? r.provider} · ${r.model}`;
    }
    case 'mt':
      return mtName(r.kind);
    case 'localOcr':
      return '本地 OCR · PaddleOCR';
    case 'degrade':
      return '降级为：本地 OCR → 翻译链';
  }
}

export function refTag(r: BackendRef, task: ChainTask): string {
  switch (r.type) {
    case 'llm':
      return task === 'translate' ? '大模型' : '视觉大模型';
    case 'mt':
      return '机器翻译';
    case 'localOcr':
      return '本地';
    case 'degrade':
      return '降级';
  }
}

export type RefState = 'ok' | 'stale' | 'nokey';

/** stale：服务商已移除、模型已取消勾选或不再是视觉模型（运行时跳过）；nokey：还没填 Key（运行时会报错并切到下一个）。 */
export function refState(r: BackendRef, task: ChainTask, providers: ProviderConfig[], keys: Record<string, boolean>, noKeyKinds: string[]): RefState {
  if (r.type === 'llm') {
    const p = providers.find((x) => x.id === r.provider);
    const m = p?.models.find((x) => x.id === r.model);
    if (!p || !m || !m.enabled) return 'stale';
    if (task !== 'translate' && !m.tags.includes('vision')) return 'stale';
    if (!noKeyKinds.includes(p.kind) && !keys[p.id]) return 'nokey';
    return 'ok';
  }
  if (r.type === 'mt') return keys[mtAccount(r.kind)] ? 'ok' : 'nokey';
  return 'ok';
}

/** 这条链可以选的全部后端（识别和一步到位只能选视觉模型）。 */
export function pool(task: ChainTask, providers: ProviderConfig[], keys: Record<string, boolean>): BackendRef[] {
  const llms: BackendRef[] = providers.flatMap((p) =>
    p.models
      .filter((m) => m.enabled && (task === 'translate' || m.tags.includes('vision')))
      .map((m) => ({ type: 'llm' as const, provider: p.id, model: m.id })),
  );
  if (task === 'translate') {
    const mts: BackendRef[] = MT_SERVICES.filter((s) => keys[mtAccount(s.kind)]).map((s) => ({ type: 'mt' as const, kind: s.kind }));
    return [...llms, ...mts];
  }
  if (task === 'recognize') return [{ type: 'localOcr' }, ...llms];
  return [...llms, { type: 'degrade' }];
}

/** 还没加进链里的候选。 */
export function available(task: ChainTask, chain: BackendRef[], providers: ProviderConfig[], keys: Record<string, boolean>): BackendRef[] {
  const have = new Set(chain.map(refKey));
  return pool(task, providers, keys).filter((r) => !have.has(refKey(r)));
}

/** 一步到位链里「降级」只能在最后。 */
export function normalize(task: ChainTask, chain: BackendRef[]): BackendRef[] {
  if (task !== 'oneshot') return chain;
  const rest = chain.filter((r) => r.type !== 'degrade');
  return rest.length === chain.length ? chain : [...rest, { type: 'degrade' }];
}

export function move<T>(arr: T[], from: number, to: number): T[] {
  if (to < 0 || to >= arr.length || from === to) return arr;
  const out = [...arr];
  const [x] = out.splice(from, 1);
  out.splice(to, 0, x);
  return out;
}
