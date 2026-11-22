<script lang="ts">
  // Markdown 结果卡片（需求 3.8、M8.1）：「编辑」视图改源码，「预览」视图实时渲染（Obsidian 方言）。
  // 复制、存为 .md 会结束截图；关闭卡片不会。
  import type { Rect } from '$lib/geometry';
  import { placeCard, type TranslateCard } from '$lib/task';
  import { renderMarkdown } from '$lib/markdown';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Seg from '$lib/ui/Seg.svelte';

  let {
    sel,
    card,
    onsrc,
    oncopy,
    onsave,
    onclose,
  }: {
    sel: Rect;
    /** 复用任务卡片的状态：src 是 Markdown 源码 */
    card: TranslateCard;
    onsrc: (v: string) => void;
    oncopy: () => void;
    onsave: () => void;
    onclose: () => void;
  } = $props();

  let height = $state(360);
  const WIDTH = 420;
  const pos = $derived(placeCard(sel, { width: WIDTH, height }, { width: innerWidth, height: innerHeight }));
  const busy = $derived(card.stage === 'markdown');
  const canAct = $derived(!busy && card.stage !== 'error' && !!card.src.trim());
  let view = $state<'edit' | 'preview'>('preview');
  const html = $derived(view === 'preview' ? renderMarkdown(card.src) : '');

  let srcEl = $state<HTMLTextAreaElement>();
  let prevEl = $state<HTMLDivElement>();
  // 流式输出时跟着滚到底
  $effect(() => {
    void card.src;
    if (!busy) return;
    if (srcEl) srcEl.scrollTop = srcEl.scrollHeight;
    if (prevEl) prevEl.scrollTop = prevEl.scrollHeight;
  });
</script>

<div
  class="card"
  role="dialog"
  tabindex="-1"
  aria-label="Markdown"
  style:left="{pos.x}px"
  style:top="{pos.y}px"
  style:width="{WIDTH}px"
  bind:clientHeight={height}
  onpointerdown={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.stopPropagation()}
>
  <header>
    <span class="chip">
      {#if busy}<span class="dot" aria-hidden="true"></span>识别中{card.backend ? ` · ${card.backend}` : ''}
      {:else if card.stage === 'error'}出错了
      {:else}Markdown{/if}
    </span>
    <span class="grow"></span>
    <Seg
      label="视图"
      options={[
        { value: 'edit', label: '编辑' },
        { value: 'preview', label: '预览' },
      ]}
      bind:value={view}
    />
    <IconButton icon="close" label="关闭  Esc" size={28} onclick={onclose} />
  </header>

  {#if card.stage === 'error'}
    <p class="err" role="alert">{card.error}</p>
  {:else if busy && !card.src}
    <div class="sk" aria-label="正在识别">
      <span style:width="40%"></span><span style:width="92%"></span><span style:width="84%"></span><span style:width="60%"></span>
    </div>
  {:else if view === 'edit'}
    <textarea
      bind:this={srcEl}
      class="field"
      aria-label="Markdown 源码"
      spellcheck="false"
      readonly={busy}
      value={card.src}
      oninput={(e) => onsrc(e.currentTarget.value)}
    ></textarea>
  {:else}
    <!-- 内容由 renderMarkdown 生成：原样 HTML 已转义，链接没有 href -->
    <div bind:this={prevEl} class="md">{@html html}</div>
  {/if}

  <footer>
    <span class="meta" title={card.meta}>{card.meta}</span>
    <Button disabled={!canAct} onclick={onsave} title="存为 .md 文件">保存 .md</Button>
    <Button variant="primary" disabled={!canAct} onclick={oncopy} title="复制 Markdown 源码">复制</Button>
  </footer>
</div>

<style>
  .card {
    position: absolute;
    z-index: 40;
    box-sizing: border-box;
    max-height: calc(100vh - 16px);
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
    from { opacity: 0; transform: translateY(6px) scale(.97); }
  }
  header, footer { display: flex; align-items: center; gap: 6px; }
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
    max-width: 200px;
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
  @keyframes pulse { from { opacity: .35; transform: scale(.8); } }
  .field, .md {
    box-sizing: border-box;
    width: 100%;
    height: 280px;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line2);
    background: var(--field);
    color: var(--text);
    padding: 10px 12px;
    overflow: auto;
  }
  .field {
    resize: vertical;
    font: 13px/20px var(--font-mono, Consolas, monospace);
    transition: border-color var(--d-hover) var(--ease-hover), box-shadow var(--d-hover) var(--ease-hover);
  }
  .field:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); }
  .md { font-size: 14px; line-height: 22px; }
  .md :global(:first-child) { margin-top: 0; }
  .md :global(h1) { font-size: 20px; line-height: 28px; margin: 14px 0 8px; }
  .md :global(h2) { font-size: 17px; line-height: 24px; margin: 14px 0 6px; }
  .md :global(h3) { font-size: 15px; margin: 12px 0 4px; }
  .md :global(p), .md :global(ul), .md :global(ol), .md :global(pre), .md :global(table), .md :global(blockquote) { margin: 0 0 10px; }
  .md :global(ul), .md :global(ol) { padding-left: 22px; }
  .md :global(code) { font-family: var(--font-mono, Consolas, monospace); font-size: 12.5px; padding: 1px 5px; border-radius: 4px; background: var(--sunken); }
  .md :global(pre) { padding: 8px 10px; border-radius: 8px; background: var(--sunken); overflow: auto; }
  .md :global(pre code) { padding: 0; background: none; }
  .md :global(pre.math), .md :global(code.math) { color: var(--accent); }
  .md :global(table) { border-collapse: collapse; font-size: 13px; }
  .md :global(th), .md :global(td) { border: 1px solid var(--line2); padding: 3px 8px; }
  .md :global(blockquote) { padding: 4px 12px; border-left: 3px solid var(--line2); color: var(--text2); }
  .md :global(blockquote.callout) { border-left-color: var(--accent); background: var(--accent-soft); border-radius: 0 8px 8px 0; }
  .md :global(.callout-title) { display: block; color: var(--text); }
  .md :global(mark) { background: rgba(250, 204, 21, .45); color: inherit; border-radius: 2px; padding: 0 2px; }
  .md :global(.wikilink), .md :global(a.link) { color: var(--accent); text-decoration: underline; text-underline-offset: 2px; cursor: default; }
  .md :global(.img) { color: var(--text3); }
  .md :global(hr) { border: none; border-top: 1px solid var(--line2); margin: 12px 0; }
  .sk { height: 280px; box-sizing: border-box; padding: 12px 4px; display: flex; flex-direction: column; gap: 12px; }
  .sk span {
    height: 12px;
    border-radius: 6px;
    background: linear-gradient(90deg, var(--sunken) 0%, var(--hover) 50%, var(--sunken) 100%);
    background-size: 200% 100%;
    animation: shimmer 1.2s linear infinite;
  }
  @keyframes shimmer { from { background-position: 100% 0; } to { background-position: -100% 0; } }
  .err { margin: 0; min-height: 60px; font-size: 13px; line-height: 20px; color: var(--danger); white-space: pre-wrap; }
  .meta { flex: 1; min-width: 0; font-size: 12px; color: var(--text3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
