<script lang="ts">
  // 设置 → 历史记录：关掉的贴图和翻译结果（可搜索）。保留方式、占用统计、缩略图、重新贴出、清空在 M10.3 加入。
  import {
    clipboardWriteText,
    historyDeleteTranslation,
    historyPins,
    historyTranslations,
    type PinRecord,
    type TransRecord,
  } from '$lib/ipc';
  import { toast } from '$lib/ui/toast.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import Field from '$lib/ui/Field.svelte';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Head from './Head.svelte';

  let tab = $state<'pin' | 'tr'>('pin');
  let query = $state('');
  let pins = $state<PinRecord[]>([]);
  let trs = $state<TransRecord[]>([]);
  let loaded = $state(false);

  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const q = query.trim();
    const t = tab;
    clearTimeout(timer);
    timer = setTimeout(async () => {
      try {
        if (t === 'pin') pins = await historyPins(q || undefined, 200);
        else trs = await historyTranslations(q || undefined, 200);
      } catch (e) {
        toast(String(e), { kind: 'error' });
      }
      loaded = true;
    }, q ? 220 : 0);
    return () => clearTimeout(timer);
  });

  const when = (ms: number) => {
    const d = new Date(ms);
    const p = (n: number) => String(n).padStart(2, '0');
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
  };
  const size = (b: number) => (b >= 1 << 20 ? `${(b / (1 << 20)).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1024))} KB`);
  const SOURCE = { capture: '截图', clipboard: '剪贴板图片', text: '文字卡片' } as const;

  async function copyTr(t: TransRecord) {
    await clipboardWriteText(t.translation);
    toast('已复制译文');
  }

  async function delTr(t: TransRecord) {
    if (await historyDeleteTranslation(t.id)) trs = trs.filter((x) => x.id !== t.id);
  }
</script>

<Head title="历史记录" desc="关掉的贴图和翻译结果都能在这里找回。" />

<div class="bar">
  <Seg
    label="历史类型"
    options={[
      { value: 'pin', label: '贴图历史' },
      { value: 'tr', label: '翻译历史' },
    ]}
    value={tab}
    onchange={(v) => (tab = v)}
  />
  <Field
    width="100%"
    aria-label="搜索历史"
    placeholder={tab === 'pin' ? '搜索贴图里识别出的文字' : '搜索原文或译文'}
    value={query}
    oninput={(e) => (query = e.currentTarget.value)}
  />
</div>

{#if tab === 'pin'}
  {#if pins.length}
    <div class="card">
      {#each pins as p (p.id)}
        <div class="hrow">
          <span class="chip">{SOURCE[p.source]}</span>
          <div class="main">
            <span class="dst">{p.text ? p.text.split('\n')[0] : `${p.width} × ${p.height}`}</span>
            <span class="meta">{when(p.closedAt || p.createdAt)} · {p.width} × {p.height} · {size(p.bytes)}</span>
          </div>
        </div>
      {/each}
    </div>
  {:else if loaded}
    <div class="card empty">{query ? '没有找到' : '还没有贴图历史'}</div>
  {/if}
{:else if trs.length}
  <div class="card">
    {#each trs as t (t.id)}
      <div class="hrow">
        <span class="chip">{t.sourceLang || '自动'} → {t.targetLang}</span>
        <div class="main">
          <span class="src">{t.source.replace(/\s+/g, ' ')}</span>
          <span class="dst">{t.translation}</span>
          <span class="meta">{when(t.updatedAt || t.createdAt)} · {t.origin === 'selection' ? '划词' : '截图'} · {t.backend}</span>
        </div>
        <div class="acts">
          <IconButton icon="copy" label="复制译文" size={28} onclick={() => copyTr(t)} />
          <IconButton icon="close" label="删除" size={28} onclick={() => delTr(t)} />
        </div>
      </div>
    {/each}
  </div>
{:else if loaded}
  <div class="card empty">{query ? '没有找到' : '还没有翻译历史'}</div>
{/if}

<style>
  .bar { display: flex; align-items: center; gap: 10px; }
  .bar :global(input) { flex: 1; }
  .card { border-radius: var(--r-panel); background: var(--card); box-shadow: var(--card-ring); }
  .empty { padding: 40px; text-align: center; font-size: 13px; color: var(--text3); }
  .hrow { display: flex; align-items: flex-start; gap: 12px; padding: 12px 16px; }
  .hrow + .hrow { border-top: 1px solid var(--line); }
  .chip {
    flex: none; margin-top: 1px; font-size: 11px; line-height: 20px; padding: 0 8px; border-radius: 6px;
    background: var(--sunken); color: var(--text2); white-space: nowrap;
  }
  .main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .src { font-size: 12px; line-height: 18px; color: var(--text2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dst { font-size: 14px; line-height: 21px; display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; white-space: pre-wrap; }
  .meta { font-size: 11px; color: var(--text3); }
  .acts { display: flex; gap: 2px; opacity: 0; transition: opacity var(--d-hover) var(--ease-hover); }
  .hrow:hover .acts, .acts:focus-within { opacity: 1; }
</style>
