<script lang="ts">
  // 公式结果卡片（需求 3.8、M8.3）：上面是 KaTeX 预览，下面是可以直接改的 LaTeX 源码，改动实时渲染。
  // 「复制 LaTeX」「复制 $$…$$」会结束截图；关闭卡片不会。
  import { untrack } from 'svelte';
  import type { Rect } from '$lib/geometry';
  import { placeCard, type TranslateCard } from '$lib/task';
  import { renderLatex, splitFormulas } from '$lib/formula';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Button from '$lib/ui/Button.svelte';

  let {
    sel,
    card,
    onsrc,
    oncopy,
    onclose,
  }: {
    sel: Rect;
    /** 复用任务卡片的状态：src 是 LaTeX 源码 */
    card: TranslateCard;
    onsrc: (v: string) => void;
    /** dollars：复制成 $$…$$ */
    oncopy: (dollars: boolean) => void;
    onclose: () => void;
  } = $props();

  let height = $state(300);
  const WIDTH = 440;
  const busy = $derived(card.stage === 'formula');
  const placed = $derived(placeCard(sel, { width: WIDTH, height }, { width: innerWidth, height: innerHeight }));
  // 识别完就钉住位置：编辑时卡片变高不会跳到选区另一侧，只在放不下时往上挪
  let pinned = $state<{ x: number; y: number } | null>(null);
  const selKey = $derived(`${sel.x},${sel.y},${sel.width},${sel.height}`);
  $effect(() => {
    void selKey; // 选区动了就跟着选区重新摆
    pinned = null;
  });
  $effect(() => {
    if (!busy && !pinned) pinned = untrack(() => placed);
  });
  const pos = $derived(
    pinned ? { x: pinned.x, y: Math.max(8, Math.min(pinned.y, innerHeight - height - 8)) } : placed,
  );
  const canAct = $derived(!busy && card.stage !== 'error' && !!card.src.trim());

  const all = $derived(splitFormulas(card.src).map((tex) => ({ tex, ...renderLatex(tex) })));
  // 流式输出时最后一个公式常常还没写完：只显示能渲染的，不闪错误
  const items = $derived(busy ? all.filter((f) => f.html) : all);
  const bad = $derived(items.filter((f) => f.error).length);
</script>

<div
  class="card"
  role="dialog"
  tabindex="-1"
  aria-label="公式"
  style:left="{pos.x}px"
  style:top="{pos.y}px"
  style:width="{WIDTH}px"
  bind:clientHeight={height}
  onpointerdown={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.stopPropagation()}
>
  <header>
    <span class="chip">
      {#if busy}<span class="dot" aria-hidden="true"></span>识别公式中{card.backend ? ` · ${card.backend}` : ''}
      {:else if card.stage === 'error'}出错了
      {:else}公式{/if}
    </span>
    <span class="grow"></span>
    <IconButton icon="close" label="关闭  Esc" size={28} onclick={onclose} />
  </header>

  {#if card.stage === 'error'}
    <p class="err" role="alert">{card.error}</p>
  {:else if !items.length && (busy || !card.src.trim())}
    <div class="sk" aria-label={busy ? '正在识别' : '没有公式'}>
      {#if busy}<span style:width="62%"></span><span style:width="38%"></span>
      {:else}<p class="empty">在下面输入 LaTeX</p>{/if}
    </div>
  {:else}
    <div class="preview" aria-label="公式预览" aria-live="polite">
      {#each items as f, i (i)}
        {#if f.html}
          <!-- KaTeX 生成的 HTML（trust 关闭，内容已转义） -->
          <div class="f">{@html f.html}</div>
        {:else}
          <div class="f bad" title={f.error}>
            <code>{f.tex}</code>
            <span class="why">{f.error}</span>
          </div>
        {/if}
      {/each}
    </div>
  {/if}

  {#if card.stage !== 'error' && (card.src || !busy)}
    <textarea
      class="field"
      aria-label="LaTeX 源码"
      spellcheck="false"
      readonly={busy}
      value={card.src}
      oninput={(e) => onsrc(e.currentTarget.value)}
    ></textarea>
  {/if}

  <footer>
    <span class="meta" class:warn={bad > 0 && !busy} title={card.meta}>
      {bad > 0 && !busy ? `${bad} 个公式写法有误` : card.meta}
    </span>
    <Button disabled={!canAct} onclick={() => oncopy(true)} title="每个公式包一对 $$，可直接粘进 Markdown / Obsidian">复制 $$</Button>
    <Button variant="primary" disabled={!canAct} onclick={() => oncopy(false)} title="复制 LaTeX 源码">复制 LaTeX</Button>
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
  @keyframes card-in { from { opacity: 0; transform: translateY(6px) scale(.97); } }
  header, footer { display: flex; align-items: center; gap: 6px; }
  .grow { flex: 1; }
  .chip {
    display: inline-flex; align-items: center; gap: 6px;
    font-size: 12px; line-height: 22px; padding: 0 8px; border-radius: 6px;
    background: var(--sunken); color: var(--text2); font-weight: 500; white-space: nowrap;
    overflow: hidden; text-overflow: ellipsis; max-width: 340px;
  }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); animation: pulse 1s var(--ease-hover) infinite alternate; flex: none; }
  @keyframes pulse { from { opacity: .35; transform: scale(.8); } }
  .preview {
    min-height: 96px;
    max-height: 260px;
    overflow: auto;
    box-sizing: border-box;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 4px;
    border-radius: var(--r-ctl);
    background: var(--field);
    border: 1px solid var(--line2);
  }
  .f { overflow-x: auto; overflow-y: hidden; animation: f-in var(--d-in) var(--ease-in) both; }
  @keyframes f-in { from { opacity: 0; transform: translateY(3px); } }
  .f :global(.katex-display) { margin: 6px 0; }
  .f :global(.katex) { font-size: 1.25em; }
  .f.bad { display: flex; flex-direction: column; gap: 2px; padding: 4px 0; }
  .f.bad code { font: 12.5px/18px var(--font-mono, Consolas, monospace); color: var(--danger); white-space: pre-wrap; word-break: break-all; }
  .why { font-size: 11px; color: var(--text3); }
  .field {
    box-sizing: border-box; width: 100%; height: 96px; resize: vertical;
    border-radius: var(--r-ctl); border: 1px solid var(--line2); background: var(--field); color: var(--text);
    font: 12.5px/19px var(--font-mono, Consolas, monospace); padding: 8px 10px;
    transition: border-color var(--d-hover) var(--ease-hover), box-shadow var(--d-hover) var(--ease-hover);
  }
  .field:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); }
  .field[readonly] { color: var(--text2); }
  .sk { height: 96px; box-sizing: border-box; padding: 18px 8px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; }
  .sk span {
    height: 18px; border-radius: 6px;
    background: linear-gradient(90deg, var(--sunken) 0%, var(--hover) 50%, var(--sunken) 100%);
    background-size: 200% 100%; animation: shimmer 1.2s linear infinite;
  }
  @keyframes shimmer { from { background-position: 100% 0; } to { background-position: -100% 0; } }
  .empty { margin: 0; font-size: 13px; color: var(--text3); }
  .err { margin: 0; font-size: 13px; line-height: 20px; color: var(--danger); white-space: pre-wrap; }
  .meta { flex: 1; min-width: 0; font-size: 12px; color: var(--text3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .meta.warn { color: var(--danger); }
</style>
