<script lang="ts">
  // 划词卡片（需求第 5 节、M7.2）：Ctrl+Alt+D 取到选中的文字后出现在鼠标旁，流式显示译文。
  // 点别处就消失（Rust 在窗口失去焦点时收起）；点图钉固定后不再自动消失，可以拖动、改原文重新翻译、
  // 改译文、切换目标语言。
  import { onMount, tick } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import {
    clipboardWriteText,
    inTauri,
    languages,
    onSelcardHide,
    onSelcardShow,
    onTaskEvent,
    selcardPending,
    selcardClose,
    selcardPin,
    selcardResize,
    taskCancel,
    translateText,
    type Lang,
  } from '$lib/ipc';
  import { config } from '$lib/config.svelte';
  import { newCard, reduce, type TranslateCard } from '$lib/task';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Select from '$lib/ui/Select.svelte';
  import ToastHost from '$lib/ui/ToastHost.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import { log } from '$lib/log';

  let card = $state<TranslateCard | null>(null);
  let pinned = $state(false);
  let target = $state('');
  let langs = $state<Lang[]>([]);
  let height = $state(0);
  let seq = 0;
  const label = inTauri() ? getCurrentWindow().label : 'selcard-dev';

  const busy = $derived(card?.stage === 'translate');
  const waiting = $derived(busy && !card?.dst);
  const who = $derived(card?.backend ? ` · ${card.backend}` : '');

  function start(text: string) {
    if (card) taskCancel(card.id);
    // 任务 id 带上窗口名：几张卡片同时在翻译时互不取消、互不串事件
    const id = `${label}-${++seq}`;
    card = newCard(id, text, 'translate');
    translateText(id, text, { target: target || undefined, selection: true }).catch((e) => {
      if (card?.id === id) card = { ...card, stage: 'error', error: String(e) };
    });
  }

  function retranslate() {
    if (!card?.src.trim()) return;
    start(card.src);
  }

  async function togglePin() {
    pinned = !pinned;
    await selcardPin(pinned);
  }

  function close() {
    if (card) taskCancel(card.id);
    selcardClose();
  }

  async function copy() {
    if (!card?.dst.trim()) return;
    try {
      await clipboardWriteText(card.dst.trim());
      toast('已复制译文');
    } catch (e) {
      toast(`复制失败：${e}`, { kind: 'error' });
    }
  }

  /** 固定后拖标题栏移动卡片（按钮、下拉框上不拖） */
  function onHeaderDown(e: PointerEvent) {
    if (!pinned || e.button !== 0 || (e.target as Element).closest('button, select')) return;
    if (inTauri()) getCurrentWindow().startDragging();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  }

  // 内容高度变了就让 Rust 调整窗口（加上阴影留白）
  $effect(() => {
    if (height > 0) selcardResize(Math.ceil(height) + 16);
  });

  /** 取走等着翻译的文字并开始（已经被取走就什么都不做） */
  async function pull() {
    const s = await selcardPending();
    if (!s) return;
    pinned = false;
    target = config.value.translation.targetLang;
    start(s.text);
    await tick();
    log('info', `划词卡片：${s.method}，${s.text.length} 字`);
  }

  onMount(() => {
    const offShow = onSelcardShow(pull);
    pull(); // 新建的卡片：页面加载前发出的通知收不到，加载完主动取一次
    const offHide = onSelcardHide(() => {
      if (card) taskCancel(card.id);
      card = null;
      pinned = false;
    });
    const offTask = onTaskEvent((e) => {
      if (card) card = reduce(card, e);
    });
    languages().then((l) => (langs = l));
    return () => {
      offShow.then((f) => f());
      offHide.then((f) => f());
      offTask.then((f) => f());
    };
  });
</script>

<svelte:window onkeydown={onKey} />

{#if card}
  <div class="card" class:pinned role="dialog" aria-label="划词翻译" bind:clientHeight={height}>
    <!-- svelte-ignore a11y_no_static_element_interactions -- 拖动标题栏移动窗口，键盘用户不需要 -->
    <header onpointerdown={onHeaderDown}>
      <span class="chip" class:busy>
        {#if busy}<span class="dot" aria-hidden="true"></span>翻译中{who}
        {:else if card.stage === 'error'}出错了
        {:else}译文{who}{/if}
      </span>
      <span class="grow"></span>
      {#if pinned && langs.length}
        <Select
          aria-label="目标语言"
          width="112px"
          options={langs.map((l) => ({ value: l.code, label: l.name }))}
          bind:value={target}
          onchange={retranslate}
        />
      {/if}
      <IconButton icon="pin" label={pinned ? '取消固定' : '固定（不再自动消失）'} size={28} on={pinned} onclick={togglePin} />
      <IconButton icon="close" label="关闭  Esc" size={28} onclick={close} />
    </header>

    {#if pinned}
      <textarea
        class="field src"
        aria-label="原文"
        spellcheck="false"
        value={card.src}
        oninput={(e) => card && (card = { ...card, src: e.currentTarget.value })}
      ></textarea>
    {:else}
      <p class="src-line" title={card.src}>{card.src}</p>
    {/if}

    {#if card.stage === 'error'}
      <p class="err" role="alert">{card.error}</p>
    {:else if waiting}
      <div class="sk" aria-label="等待译文"><span style:width="90%"></span><span style:width="64%"></span></div>
    {:else if pinned}
      <textarea
        class="field dst"
        aria-label="译文"
        spellcheck="false"
        readonly={busy}
        value={card.dst}
        oninput={(e) => card && (card = { ...card, dst: e.currentTarget.value })}
      ></textarea>
    {:else}
      <p class="dst-text">{card.dst}</p>
    {/if}

    <footer>
      <span class="meta" title={card.meta}>{card.meta}</span>
      {#if pinned}
        <IconButton icon="refresh" label="按当前原文重新翻译" size={28} disabled={busy || !card.src.trim()} onclick={retranslate} />
      {/if}
      <Button variant="primary" disabled={busy || !card.dst.trim()} onclick={copy}>复制</Button>
    </footer>
  </div>
{/if}

<ToastHost />

<style>
  :global(html), :global(body) {
    background: transparent;
    overflow: hidden;
    margin: 0;
  }
  .card {
    box-sizing: border-box;
    width: 364px;
    margin: 8px;
    max-height: 624px;
    padding: 10px 12px 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-radius: 14px;
    background: var(--glass2);
    backdrop-filter: var(--blur-2);
    box-shadow: var(--elev);
    color: var(--text);
    animation: card-in var(--d-in) var(--ease-in) both;
  }
  .card.pinned { box-shadow: 0 0 0 1.5px var(--accent), var(--elev); }
  @keyframes card-in {
    from { opacity: 0; transform: translateY(4px) scale(.98); }
  }
  header, footer {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .pinned header { cursor: move; }
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
    max-width: 180px;
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
  .src-line {
    margin: 0;
    font-size: 12px;
    line-height: 18px;
    color: var(--text3);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }
  .dst-text {
    margin: 0;
    font-size: 15px;
    line-height: 24px;
    font-weight: 500;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 432px;
    overflow: auto;
    user-select: text;
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
    padding: 6px 9px;
    transition:
      border-color var(--d-hover) var(--ease-hover),
      box-shadow var(--d-hover) var(--ease-hover);
  }
  .field:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .src { height: 64px; font-size: 13px; line-height: 20px; color: var(--text2); }
  .dst { height: 112px; font-size: 15px; line-height: 24px; font-weight: 500; }
  .sk {
    padding: 6px 2px;
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
