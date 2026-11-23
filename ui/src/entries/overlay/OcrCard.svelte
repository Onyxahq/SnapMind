<script lang="ts">
  // 识别文字卡片（需求 3.8、M4.4）：按段落 / 按行切换、可编辑；复制或贴图后结束截图，关闭卡片不会。
  // 本地 OCR 模型第一次用时在这里下载（约 21 MB，PaddlePaddle 官方仓库，逐个校验 SHA-256）。
  import type { Rect } from '$lib/geometry';
  import type { OcrStatus } from '$lib/ipc';
  import { floatCard } from '$lib/floatCard';
  import { ocrMeta, progressLabel, type OcrCard } from '$lib/ocr';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Seg from '$lib/ui/Seg.svelte';

  let {
    sel,
    card,
    model,
    ontext,
    onmode,
    ondownload,
    oncopy,
    onpin,
    onclose,
  }: {
    sel: Rect;
    card: OcrCard;
    /** 模型下载状态（stage = model 时显示） */
    model: OcrStatus | null;
    ontext: (v: string) => void;
    onmode: (byLine: boolean) => void;
    ondownload: () => void;
    oncopy: () => void;
    onpin: () => void;
    onclose: () => void;
  } = $props();

  const WIDTH = 380;
  const canAct = $derived(card.stage === 'done' && !!card.text.trim());
  const empty = $derived(card.stage === 'done' && !!card.result && card.result.lines.length === 0);
  const pct = $derived(model && model.total ? Math.min(100, (model.received / model.total) * 100) : 0);

  let mode = $state<'para' | 'line'>('para');
  $effect(() => {
    mode = card.byLine ? 'line' : 'para';
  });
</script>

<div
  class="card"
  role="dialog"
  tabindex="-1"
  aria-label="识别文字"
  use:floatCard={{ sel, width: WIDTH }}
  onpointerdown={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.stopPropagation()}
>
  <header>
    <span class="chip" class:busy={card.stage === 'run'}>
      {#if card.stage === 'run'}<span class="dot" aria-hidden="true"></span>识别中 · 本地 OCR
      {:else if card.stage === 'model'}需要下载识别模型
      {:else if card.stage === 'error'}出错了
      {:else}识别文字{/if}
    </span>
    <span class="grow"></span>
    {#if card.stage === 'done' && !empty}
      <Seg
        label="排列方式"
        options={[
          { value: 'para', label: '按段落' },
          { value: 'line', label: '按行' },
        ]}
        bind:value={mode}
        onchange={(v) => onmode(v === 'line')}
      />
    {/if}
    <IconButton icon="close" label="关闭  Esc" size={28} onclick={onclose} />
  </header>

  {#if card.stage === 'run'}
    <div class="sk" aria-label="正在识别">
      <span style:width="94%"></span>
      <span style:width="82%"></span>
      <span style:width="60%"></span>
      <span style:width="88%"></span>
      <span style:width="40%"></span>
    </div>
  {:else if card.stage === 'model'}
    <div class="model">
      <p>本地 OCR 第一次使用要先下载识别模型（约 21 MB，来自 PaddlePaddle 官方仓库），下载后离线可用。</p>
      {#if model?.downloading}
        <div class="bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)}>
          <span style:width="{pct}%"></span>
        </div>
        <span class="cap">正在下载 {progressLabel(model)}</span>
      {:else}
        {#if model?.error}<p class="err" role="alert">{model.error}</p>{/if}
        <div><Button variant="primary" onclick={ondownload}>{model?.error ? '重新下载' : '下载模型'}</Button></div>
      {/if}
    </div>
  {:else if card.stage === 'error'}
    <p class="err" role="alert">{card.error}</p>
  {:else if empty}
    <p class="none">选区里没有识别到文字</p>
  {:else}
    <textarea
      class="field"
      class:mono={card.byLine}
      aria-label="识别出的文字"
      spellcheck="false"
      value={card.text}
      oninput={(e) => ontext(e.currentTarget.value)}
    ></textarea>
    {#if !card.pick}
      <span class="cap">单击检测框挑选行，Ctrl+A 全选，右键划过选一片</span>
    {/if}
  {/if}

  <footer>
    <span class="meta" title={ocrMeta(card)}>{ocrMeta(card)}</span>
    <Button disabled={!canAct} onclick={onpin} title="把文字贴到屏幕上">贴图</Button>
    <Button variant="primary" disabled={!canAct} onclick={oncopy}>复制</Button>
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
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse 1s var(--ease-hover) infinite alternate;
  }
  @keyframes pulse {
    from { opacity: .35; transform: scale(calc(1 - .2 * var(--m))); }
  }
  .field {
    box-sizing: border-box;
    width: 100%;
    height: 200px;
    resize: vertical;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line2);
    background: var(--field);
    color: var(--text);
    font: inherit;
    font-size: 14px;
    line-height: 22px;
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
  .field.mono { white-space: pre; overflow-x: auto; }
  /* 拉伸卡片时文字框填满 */
  .field, .sk { flex: 1 1 auto; min-height: 60px; }
  .card:global(.sized) .field { resize: none; }
  .cap {
    font-size: 11px;
    color: var(--text3);
    letter-spacing: .04em;
  }
  .sk {
    height: 200px;
    box-sizing: border-box;
    padding: 12px 4px;
    display: flex;
    flex-direction: column;
    gap: 12px;
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
  .model {
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-size: 13px;
    line-height: 20px;
    color: var(--text2);
  }
  .model p { margin: 0; }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--sunken);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
    transition: width 200ms var(--ease-hover);
  }
  .err {
    margin: 0;
    font-size: 13px;
    line-height: 20px;
    color: var(--danger);
    white-space: pre-wrap;
  }
  .none {
    margin: 0;
    padding: 24px 0;
    text-align: center;
    font-size: 13px;
    color: var(--text3);
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
