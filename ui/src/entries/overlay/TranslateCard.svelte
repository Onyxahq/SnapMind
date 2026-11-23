<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 翻译结果卡片（需求 3.8）：原文和译文都可编辑；改了原文点 ↻ 重新翻译；
  // 复制译文 / 贴图会结束截图，关闭卡片不会。
  import type { Rect } from '$lib/geometry';
  import { stageLabel, type TranslateCard } from '$lib/task';
  import { floatCard } from '$lib/floatCard';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Seg from '$lib/ui/Seg.svelte';

  let {
    sel,
    card,
    bilingual = false,
    overlay = false,
    onmode,
    onsrc,
    ondst,
    onretranslate,
    oncopy,
    onpin,
    onclose,
  }: {
    sel: Rect;
    card: TranslateCard;
    /** 复制 / 贴图时用双语对照 */
    bilingual?: boolean;
    /** 原位覆盖模式：复制 / 贴图交出的是画上译文的截图 */
    overlay?: boolean;
    onmode: (overlay: boolean) => void;
    onsrc: (v: string) => void;
    ondst: (v: string) => void;
    onretranslate: () => void;
    oncopy: () => void;
    onpin: () => void;
    onclose: () => void;
  } = $props();

  const WIDTH = 380;
  const busy = $derived(card.stage === 'recognize' || card.stage === 'oneshot' || card.stage === 'translate');
  const streamingSrc = $derived(card.stage === 'recognize' || card.stage === 'oneshot');
  const waitingDst = $derived(card.stage === 'translate' && !card.dst);
  const canAct = $derived(!busy && !!card.dst.trim());

  let mode = $state<'card' | 'overlay'>('card');
  $effect(() => {
    mode = overlay ? 'overlay' : 'card';
  });

  let dstEl = $state<HTMLTextAreaElement>();
  // 流式输出时译文框跟着滚到底
  $effect(() => {
    void card.dst;
    if (busy && dstEl) dstEl.scrollTop = dstEl.scrollHeight;
  });
</script>

<div
  class="card"
  role="dialog"
  tabindex="-1"
  aria-label={t('翻译结果')}
  use:floatCard={{ sel, width: WIDTH }}
  onpointerdown={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.stopPropagation()}
>
  <header>
    <span class="chip" class:busy>
      {#if busy}<span class="dot" aria-hidden="true"></span>{/if}
      {stageLabel(card)}
    </span>
    <span class="grow"></span>
    <Seg
      label={t('结果呈现')}
      options={[
        { value: 'card', label: t('卡片') },
        { value: 'overlay', label: t('覆盖') },
      ]}
      bind:value={mode}
      onchange={(v) => onmode(v === 'overlay')}
    />
    <IconButton icon="close" label={t('关闭 Esc')} size={28} onclick={onclose} />
  </header>

  <label class="blk">
    <span class="cap">{t('原文 · 可编辑')}</span>
    <textarea
      class="field src"
      aria-label={t('原文')}
      spellcheck="false"
      readonly={streamingSrc}
      placeholder={streamingSrc ? t('正在识别…') : ''}
      value={card.src}
      oninput={(e) => onsrc(e.currentTarget.value)}
    ></textarea>
  </label>

  <div class="blk">
    <span class="cap">{t('译文 · 可编辑')}</span>
    {#if card.stage === 'error'}
      <p class="err" role="alert">{card.error}</p>
    {:else if waitingDst || streamingSrc}
      <div class="sk" aria-label={t('等待译文')}>
        <span style:width="92%"></span>
        <span style:width="70%"></span>
        <span style:width="48%"></span>
      </div>
    {:else}
      <textarea
        bind:this={dstEl}
        class="field dst"
        aria-label={t('译文')}
        spellcheck="false"
        readonly={busy}
        value={card.dst}
        oninput={(e) => ondst(e.currentTarget.value)}
      ></textarea>
    {/if}
  </div>

  <footer>
    <span class="meta" title={card.meta}>{card.meta}</span>
    <IconButton
      icon="refresh"
      label={t('按当前原文重新翻译')}
      size={30}
      disabled={busy || !card.src.trim()}
      onclick={onretranslate}
    />
    {#if overlay}
      <Button disabled={!canAct} onclick={onpin} title={t('把画上译文的截图贴到屏幕上')}>{t('贴图')}</Button>
      <Button variant="primary" disabled={!canAct} onclick={oncopy} title={t('复制画上译文的截图')}>{t('复制图片')}</Button>
    {:else}
      <Button disabled={!canAct} onclick={onpin} title={bilingual ? t('把原文和译文对照贴到屏幕上') : t('把译文贴到屏幕上')}>{t('贴图')}</Button>
      <Button variant="primary" disabled={!canAct} onclick={oncopy}>{bilingual ? t('复制双语') : t('复制译文')}</Button>
    {/if}
  </footer>
</div>

<style>
  .card {
    position: absolute;
    z-index: 40;
    box-sizing: border-box;
    max-height: calc(100vh - 16px);
    overflow: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    border-radius: 14px;
    background: var(--glass2);
    backdrop-filter: var(--blur-2);
    box-shadow: var(--elev);
    color: var(--text);
    user-select: text;
    animation: card-in var(--d-in) var(--ease-in) both;
    transform-origin: top left;
  }
  @keyframes card-in {
    from { opacity: 0; transform: translateY(calc(6px * var(--m))) scale(calc(1 - .03 * var(--m))); }
  }
  header,
  footer {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .grow { flex: 1; }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    line-height: 22px;
    padding: 0 8px;
    border-radius: 6px;
    background: var(--sunken);
    color: var(--text2);
    font-weight: 500;
    white-space: nowrap;
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse 1s var(--ease-hover) infinite alternate;
    flex: none;
  }
  @keyframes pulse {
    from { opacity: .35; transform: scale(calc(1 - .2 * var(--m))); }
  }
  .blk {
    display: flex;
    flex-direction: column;
    gap: 4px;
    /* 拉伸卡片时原文、译文按 1 : 2 分高度 */
    flex: 1 1 auto;
    min-height: 0;
  }
  .blk + .blk { flex-grow: 2; }
  .blk > .field, .blk > .sk { flex: 1 1 auto; min-height: 44px; }
  .card:global(.sized) .field { resize: none; }
  .cap {
    font-size: 11px;
    color: var(--text3);
    letter-spacing: .06em;
  }
  .field {
    box-sizing: border-box;
    width: 100%;
    resize: vertical;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line2);
    background: var(--field);
    color: var(--text);
    font: inherit;
    padding: 8px 10px;
    transition:
      border-color var(--d-hover) var(--ease-hover),
      box-shadow var(--d-hover) var(--ease-hover);
  }
  .field:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .src {
    height: 84px;
    font-size: 13px;
    line-height: 20px;
    color: var(--text2);
  }
  .dst {
    height: 132px;
    font-size: 15px;
    line-height: 24px;
    font-weight: 500;
  }
  .sk {
    height: 132px;
    box-sizing: border-box;
    padding: 12px 4px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .sk span {
    height: 12px;
    border-radius: 6px;
    background: linear-gradient(90deg, var(--sunken) 0%, var(--hover) 50%, var(--sunken) 100%);
    background-size: 200% 100%;
    animation: shimmer 1.2s linear infinite;
  }
  @keyframes shimmer {
    from { background-position: 100% 0; }
    to { background-position: -100% 0; }
  }
  .err {
    margin: 0;
    min-height: 60px;
    font-size: 13px;
    line-height: 20px;
    color: var(--danger);
    white-space: pre-wrap;
  }
  .meta {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--text3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
