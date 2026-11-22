<script lang="ts">
  // 翻译结果卡片（需求 3.8）：原文和译文都可编辑；改了原文点 ↻ 重新翻译；
  // 复制译文 / 贴图会结束截图，关闭卡片不会。
  import type { Rect } from '$lib/geometry';
  import { placeCard, stageLabel, type TranslateCard } from '$lib/task';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Button from '$lib/ui/Button.svelte';

  let {
    sel,
    card,
    onsrc,
    ondst,
    onretranslate,
    oncopy,
    onpin,
    onclose,
  }: {
    sel: Rect;
    card: TranslateCard;
    onsrc: (v: string) => void;
    ondst: (v: string) => void;
    onretranslate: () => void;
    oncopy: () => void;
    onpin: () => void;
    onclose: () => void;
  } = $props();

  let height = $state(320);
  const WIDTH = 380;
  const pos = $derived(placeCard(sel, { width: WIDTH, height }, { width: innerWidth, height: innerHeight }));
  const busy = $derived(card.stage === 'recognize' || card.stage === 'translate');
  const streamingSrc = $derived(card.stage === 'recognize');
  const waitingDst = $derived(card.stage === 'translate' && !card.dst);
  const canAct = $derived(!busy && !!card.dst.trim());

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
  aria-label="翻译结果"
  style:left="{pos.x}px"
  style:top="{pos.y}px"
  style:width="{WIDTH}px"
  bind:clientHeight={height}
  onpointerdown={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.stopPropagation()}
>
  <header>
    <span class="chip" class:busy>
      {#if busy}<span class="dot" aria-hidden="true"></span>{/if}
      {stageLabel(card)}
    </span>
    <span class="grow"></span>
    <IconButton icon="close" label="关闭  Esc" size={28} onclick={onclose} />
  </header>

  <label class="blk">
    <span class="cap">原文 · 可编辑</span>
    <textarea
      class="field src"
      aria-label="原文"
      spellcheck="false"
      readonly={streamingSrc}
      placeholder={streamingSrc ? '正在识别…' : ''}
      value={card.src}
      oninput={(e) => onsrc(e.currentTarget.value)}
    ></textarea>
  </label>

  <div class="blk">
    <span class="cap">译文 · 可编辑</span>
    {#if card.stage === 'error'}
      <p class="err" role="alert">{card.error}</p>
    {:else if waitingDst || streamingSrc}
      <div class="sk" aria-label="等待译文">
        <span style:width="92%"></span>
        <span style:width="70%"></span>
        <span style:width="48%"></span>
      </div>
    {:else}
      <textarea
        bind:this={dstEl}
        class="field dst"
        aria-label="译文"
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
      label="按当前原文重新翻译"
      size={30}
      disabled={busy || !card.src.trim()}
      onclick={onretranslate}
    />
    <Button disabled={!canAct} onclick={onpin}>贴图</Button>
    <Button variant="primary" disabled={!canAct} onclick={oncopy}>复制译文</Button>
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
    from { opacity: 0; transform: translateY(6px) scale(.97); }
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
    from { opacity: .35; transform: scale(.8); }
  }
  .blk {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
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
