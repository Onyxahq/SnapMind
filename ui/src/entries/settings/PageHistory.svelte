<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 设置 → 历史记录（需求第 7 节、M10.3）：保留方式（不限制 / 按条数 / 按空间）、占用统计、
  // 贴图历史（缩略图，重新贴出 / 复制 / 删除）和翻译历史（复制译文 / 贴到屏幕 / 删除），搜索、清空。
  import { onMount } from 'svelte';
  import { flip } from '$lib/motion';
  import { fade } from 'svelte/transition';
  import {
    clipboardWriteText,
    historyClear,
    historyCopyPin,
    historyDeletePin,
    historyDeleteTranslation,
    historyPins,
    historyPinText,
    historyRepin,
    historyTranslations,
    historyUsage,
    openFolder,
    thumbUrl,
    type HistoryConfig,
    type HistoryUsage,
    type PinRecord,
    type TransRecord,
  } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import Field from '$lib/ui/Field.svelte';
  import Button from '$lib/ui/Button.svelte';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Head from './Head.svelte';

  const h = $derived(config.value.history);
  let usage = $state<HistoryUsage | null>(null);
  let tab = $state<'pin' | 'tr'>('pin');
  let query = $state('');
  let pins = $state<PinRecord[]>([]);
  let trs = $state<TransRecord[]>([]);
  let loaded = $state(false);
  /** 「清空」要点两次 */
  let armClear = $state(false);

  async function refreshUsage() {
    try {
      usage = await historyUsage();
    } catch {
      usage = null;
    }
  }
  onMount(refreshUsage);

  let timer: ReturnType<typeof setTimeout> | undefined;
  async function load() {
    try {
      const q = query.trim() || undefined;
      if (tab === 'pin') pins = await historyPins(q, 300);
      else trs = await historyTranslations(q, 300);
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
    loaded = true;
  }
  $effect(() => {
    void tab;
    const q = query.trim();
    armClear = false;
    clearTimeout(timer);
    timer = setTimeout(load, q ? 220 : 0);
    return () => clearTimeout(timer);
  });

  async function save(patch: Partial<HistoryConfig>) {
    try {
      await updateConfig({ history: patch });
      // 收紧保留方式后 Rust 会在后台清理：稍等再刷新列表和统计
      setTimeout(() => {
        refreshUsage();
        load();
      }, 400);
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  // ---- 占用 ----
  const MB = 1024 * 1024;
  const fmt = (b: number) => (b >= 1024 * MB ? `${(b / 1024 / MB).toFixed(2)} GB` : b >= MB ? `${(b / MB).toFixed(1)} MB` : `${Math.max(0, Math.round(b / 1024))} KB`);
  const total = $derived(usage ? usage.pins.bytes + usage.translations.bytes : 0);
  /** 进度条的分母：按空间时是上限，其余是总量 */
  const scale = $derived(h.mode === 'size' ? Math.max(h.maxMb * MB, total, 1) : Math.max(total, 1));
  const note = $derived(
    h.mode === 'unlimited'
      ? t('全部保留，需要时手动清理')
      : h.mode === 'count'
        ? t('贴图和翻译各最多保留这么多条，超出后删除最早的')
        : t('超出后删除最早的记录，贴图优先清理'),
  );

  // ---- 列表 ----
  const when = (ms: number) => {
    const d = new Date(ms);
    const p = (n: number) => String(n).padStart(2, '0');
    return `${d.getMonth() + 1}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
  };
  const pinTitle = (p: PinRecord) => (p.text.trim() ? p.text.trim().split('\n')[0] : p.source === 'clipboard' ? t('剪贴板图片') : t('截图'));

  async function repin(p: PinRecord) {
    try {
      await historyRepin(p.id);
      pins = pins.filter((x) => x.id !== p.id);
      refreshUsage();
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  async function copyPin(p: PinRecord) {
    try {
      await historyCopyPin(p.id);
      toast(t('已复制图片'));
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  async function delPin(p: PinRecord) {
    if (await historyDeletePin(p.id)) {
      pins = pins.filter((x) => x.id !== p.id);
      refreshUsage();
    }
  }

  async function delTr(t: TransRecord) {
    if (await historyDeleteTranslation(t.id)) {
      trs = trs.filter((x) => x.id !== t.id);
      refreshUsage();
    }
  }

  async function clear() {
    if (!armClear) {
      armClear = true;
      return;
    }
    armClear = false;
    try {
      const n = await historyClear(tab === 'pin' ? 'pins' : 'translations');
      if (tab === 'pin') pins = [];
      else trs = [];
      refreshUsage();
      toast(tab === 'pin' ? t('已清空 {n} 条贴图历史', { n }) : t('已清空 {n} 条翻译历史', { n }));
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }
</script>

<Head title={t('历史记录')} desc={t('关掉的贴图和翻译结果都能在这里找回。')} />

<Card>
  <Row title={t('保留方式')} desc={note}>
    <Seg
      label={t('保留方式')}
      options={[
        { value: 'unlimited', label: t('不限制') },
        { value: 'count', label: t('按条数') },
        { value: 'size', label: t('按空间') },
      ]}
      value={h.mode}
      onchange={(mode: HistoryConfig['mode']) => save({ mode })}
    />
  </Row>
  {#if h.mode === 'count'}
    <Row title={t('最多保留')} desc={t('每一类分别计算')}>
      <div class="num">
        <Field
          width="96px"
          type="number"
          min="1"
          aria-label={t('保留条数')}
          value={String(h.maxCount)}
          onchange={(e) => save({ maxCount: Math.max(1, Math.round(Number(e.currentTarget.value) || 1)) })}
        />
        <span>{t('条')}</span>
      </div>
    </Row>
  {:else if h.mode === 'size'}
    <Row title={t('最多占用')} desc={t('贴图和翻译合计')}>
      <div class="num">
        <Field
          width="96px"
          type="number"
          min="10"
          aria-label={t('空间上限')}
          value={String(h.maxMb)}
          onchange={(e) => save({ maxMb: Math.max(10, Math.round(Number(e.currentTarget.value) || 10)) })}
        />
        <span>MB</span>
      </div>
    </Row>
  {/if}
  <div class="usage">
    <div class="ut">
      <b class="lbl">{t('占用空间')}</b>
      <b class="sum">{fmt(total)}</b>
      {#if h.mode === 'size'}<span class="lim">/ {fmt(h.maxMb * MB)} · {Math.min(100, Math.round((total / (h.maxMb * MB)) * 100))}%</span>{/if}
    </div>
    <div class="meter" aria-hidden="true">
      <span class="a" style:width="{((usage?.pins.bytes ?? 0) / scale) * 100}%"></span>
      <span class="b" style:width="{((usage?.translations.bytes ?? 0) / scale) * 100}%"></span>
    </div>
    <div class="legend">
      <span><i class="a"></i>{t('贴图 {v} 张 · {fmt}', { v: usage?.pins.count ?? 0, fmt: fmt(usage?.pins.bytes ?? 0) })}</span>
      <span><i class="b"></i>{t('翻译 {v} 条 · {fmt}', { v: usage?.translations.count ?? 0, fmt: fmt(usage?.translations.bytes ?? 0) })}</span>
      <span class="grow"></span>
      <Button variant="ghost" icon="folder" onclick={() => usage && openFolder(usage.dir)}>{t('打开所在文件夹')}</Button>
    </div>
  </div>
</Card>

<div class="bar">
  <Seg
    label={t('历史类型')}
    options={[
      { value: 'pin', label: t('贴图历史') },
      { value: 'tr', label: t('翻译历史') },
    ]}
    value={tab}
    onchange={(v) => (tab = v)}
  />
  <Field
    width="100%"
    aria-label={t('搜索历史')}
    placeholder={tab === 'pin' ? t('搜索贴图里的文字') : t('搜索原文或译文')}
    value={query}
    oninput={(e) => (query = e.currentTarget.value)}
  />
  <Button variant="danger" onclick={clear} onblur={() => (armClear = false)}>{armClear ? t('再点一次清空') : t('清空')}</Button>
</div>

{#if tab === 'pin'}
  {#if pins.length}
    <div class="grid">
      {#each pins as p (p.id)}
        <div class="thumb" animate:flip={{ duration: 180 }} out:fade={{ duration: 120 }}>
          <div class="img">
            {#if thumbUrl(p.id)}<img src={thumbUrl(p.id)} alt="" loading="lazy" />{:else}<span class="ph"></span>{/if}
          </div>
          <div class="tt">
            <b title={pinTitle(p)}>{pinTitle(p)}</b>
            <span>{when(p.closedAt || p.createdAt)} · {p.width}×{p.height} · {fmt(p.bytes)}</span>
          </div>
          <div class="acts">
            <IconButton icon="pin" label={t('重新贴出')} size={28} onclick={() => repin(p)} />
            <IconButton icon="copy" label={t('复制图片')} size={28} onclick={() => copyPin(p)} />
            <IconButton icon="close" label={t('删除')} size={28} onclick={() => delPin(p)} />
          </div>
        </div>
      {/each}
    </div>
  {:else if loaded}
    <div class="card empty">{query ? t('没有找到') : t('还没有贴图历史——关掉的贴图会出现在这里')}</div>
  {/if}
{:else if trs.length}
  <div class="card">
    {#each trs as r (r.id)}
      <div class="hrow" animate:flip={{ duration: 180 }} out:fade={{ duration: 120 }}>
        <span class="chip">{r.sourceLang || t('自动')} → {r.targetLang}</span>
        <div class="main">
          <span class="src">{r.source.replace(/\s+/g, ' ')}</span>
          <span class="dst">{r.translation}</span>
          <span class="meta">{when(r.updatedAt || r.createdAt)} · {r.origin === 'selection' ? t('划词') : t('截图')} · {r.backend}</span>
        </div>
        <div class="racts">
          <IconButton icon="copy" label={t('复制译文')} size={28} onclick={async () => (await clipboardWriteText(r.translation), toast(t('已复制译文')))} />
          <IconButton icon="pin" label={t('贴到屏幕')} size={28} onclick={() => historyPinText(r.translation).catch((e) => toast(String(e), { kind: 'error' }))} />
          <IconButton icon="close" label={t('删除')} size={28} onclick={() => delTr(r)} />
        </div>
      </div>
    {/each}
  </div>
{:else if loaded}
  <div class="card empty">{query ? t('没有找到') : t('还没有翻译历史')}</div>
{/if}

<style>
  .num { display: flex; align-items: center; gap: 8px; font-size: 13px; color: var(--text2); }
  .usage { display: flex; flex-direction: column; gap: 10px; padding: 12px 16px 14px; border-top: 1px solid var(--line); }
  .ut { display: flex; align-items: baseline; gap: 8px; }
  .ut .lbl { flex: 1; font-size: 14px; font-weight: 500; }
  .ut .sum { font-size: 18px; font-variant-numeric: tabular-nums; }
  .lim { font-size: 12px; color: var(--text3); }
  .meter { display: flex; height: 8px; border-radius: 4px; overflow: hidden; background: var(--sunken); }
  .meter span { transition: width 320ms var(--ease-hover); }
  .meter .a, .legend i.a { background: var(--accent); }
  .meter .b, .legend i.b { background: var(--text3); }
  .legend { display: flex; align-items: center; gap: 18px; font-size: 12px; color: var(--text2); }
  .legend span { display: inline-flex; align-items: center; gap: 6px; }
  .legend i { width: 8px; height: 8px; border-radius: 2px; }
  .grow { flex: 1; }
  .bar { display: flex; align-items: center; gap: 10px; }
  .bar :global(input) { flex: 1; }
  .card { border-radius: var(--r-panel); background: var(--card); box-shadow: var(--card-ring); }
  .empty { padding: 40px; text-align: center; font-size: 13px; color: var(--text3); }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 12px; }
  .thumb {
    position: relative; border-radius: var(--r-panel); background: var(--card); box-shadow: var(--card-ring);
    overflow: hidden; transition: box-shadow var(--d-hover) var(--ease-hover), transform var(--d-hover) var(--ease-hover);
  }
  .thumb:hover { box-shadow: var(--card-ring), var(--elev-sm); transform: translateY(-1px); }
  .img { height: 110px; margin: 8px 8px 0; border-radius: 6px; overflow: hidden; background: var(--sunken); box-shadow: 0 0 0 1px var(--line); }
  .img img { width: 100%; height: 100%; object-fit: cover; object-position: top center; display: block; }
  .ph { display: block; width: 100%; height: 100%; background: repeating-linear-gradient(180deg, var(--hover) 0 6px, transparent 6px 14px); }
  .tt { display: flex; flex-direction: column; gap: 2px; padding: 8px 12px 10px; }
  .tt b { font-size: 13px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tt span { font-size: 11px; color: var(--text3); }
  .acts {
    position: absolute; top: 12px; right: 12px; display: flex; gap: 2px; padding: 2px; border-radius: 8px;
    background: var(--glass2); backdrop-filter: var(--blur-1); box-shadow: var(--elev-sm);
    opacity: 0; transform: translateY(-2px); transition: opacity var(--d-hover) var(--ease-hover), transform var(--d-hover) var(--ease-hover);
  }
  .thumb:hover .acts, .acts:focus-within { opacity: 1; transform: none; }
  .hrow { display: flex; align-items: flex-start; gap: 12px; padding: 12px 16px; }
  .hrow + .hrow { border-top: 1px solid var(--line); }
  .chip { flex: none; margin-top: 1px; font-size: 11px; line-height: 20px; padding: 0 8px; border-radius: 6px; background: var(--sunken); color: var(--text2); white-space: nowrap; }
  .main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
  .src { font-size: 12px; line-height: 18px; color: var(--text2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dst { font-size: 14px; line-height: 21px; display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; white-space: pre-wrap; }
  .meta { font-size: 11px; color: var(--text3); }
  .racts { display: flex; gap: 2px; opacity: 0; transition: opacity var(--d-hover) var(--ease-hover); }
  .hrow:hover .racts, .racts:focus-within { opacity: 1; }
</style>
