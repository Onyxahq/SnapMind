<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 表格结果卡片（需求 3.8、M8.2、F5）：「表格」视图是渲染好的 HTML 表格，默认只看（预览），点笔切到编辑，
  // 单元格直接改（选哪种会记住）；「Markdown」「CSV」视图里改源码，切回表格时解析回来。
  // 表格视图两个复制：「复制纯文本」是制表符分隔的文字；「复制」同时放 HTML 和制表符文本，
  // 粘贴进 Excel / Word 直接成表格、保留表头和合并单元格。Markdown / CSV 视图只有一个「复制」，复制源码。
  // 复制和导出 .xlsx 会结束截图。卡片可拖动、可拉伸（floatCard）。
  // 表格框右下角（两条滚动条相交处）的按钮把卡片展开到看得全，再点、Esc、点暗处、双击标题栏还原。
  import type { Rect } from '$lib/geometry';
  import type { OcrStatus } from '$lib/ipc';
  import { onDestroy, tick } from 'svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import { fade } from 'svelte/transition';
  import { floatCard } from '$lib/floatCard';
  import { progressLabel } from '$lib/ocr';
  import { parseCsv, parseMarkdown, toCsv, toMarkdown, toTsv, type TableCardState, type TableView, type TRows } from '$lib/table';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Seg from '$lib/ui/Seg.svelte';

  let {
    sel,
    card,
    model,
    mode,
    full = $bindable(false),
    cellWidth,
    onmode,
    onrows,
    ondownload,
    oncopy,
    onexport,
    onclose,
  }: {
    sel: Rect;
    card: TableCardState;
    model: OcrStatus | null;
    /** 表格视图是只看还是可以改单元格（记在设置里） */
    mode: 'preview' | 'edit';
    /** 展开到看得全 */
    full?: boolean;
    /** 展开时单元格最宽多少像素，超过就换行；0 不限 */
    cellWidth: number;
    onmode: (mode: 'preview' | 'edit') => void;
    onrows: (rows: TRows) => void;
    ondownload: () => void;
    /** text：复制纯文本（当前视图的文字）；excel：复制成 Excel 能直接粘贴的表格（rows 是最新的数据） */
    oncopy: (kind: 'text' | 'excel', rows: TRows, text: string) => void;
    onexport: () => void;
    onclose: () => void;
  } = $props();

  const WIDTH = 500;
  const canAct = $derived(card.stage === 'done' && card.rows.length > 0);
  const pct = $derived(model && model.total ? Math.min(100, (model.received / model.total) * 100) : 0);

  let view = $state<TableView>('grid');
  /** 源码视图里正在编辑的文字，以及刚生成时的样子（没改过就不解析回去——Markdown 表示不了合并单元格） */
  let source = $state('');
  let generated = '';
  /** 换了一份数据就重建表格（单元格是 contenteditable，编辑时不重绘） */
  let version = $state(0);

  /** 源码视图里改过的源码解析回表格（没改过、或解析不出来就保留原来的），返回最新的数据 */
  function commitSource(): TRows {
    if (view !== 'grid' && source !== generated) {
      const parsed = view === 'markdown' ? parseMarkdown(source) : parseCsv(source);
      if (parsed) {
        onrows(parsed);
        version++;
        generated = source;
        return parsed;
      }
    }
    return card.rows;
  }

  function switchView(next: TableView) {
    commitSource();
    if (next !== 'grid') source = generated = next === 'markdown' ? toMarkdown(card.rows) : toCsv(card.rows);
    view = next;
  }

  function editCell(r: number, c: number, text: string) {
    const rows = card.rows.map((row) => row.slice());
    rows[r][c] = { ...rows[r][c], text };
    onrows(rows);
  }

  /** 双击单元格临时编辑这一格（不改记住的预览 / 编辑） */
  let single = $state('');
  async function editOne(e: MouseEvent, r: number, c: number) {
    if (mode === 'edit') return;
    const td = e.currentTarget as HTMLElement;
    single = `${r},${c}`;
    await tick();
    td.focus();
    const s = getSelection();
    s?.selectAllChildren(td);
    s?.collapseToEnd();
  }

  let body = $state<HTMLElement>();
  let grid = $state<HTMLElement>();
  /** 表格框放不下整张表；右边 / 下边还有没滚到的内容 */
  let overflow = $state(false);
  let moreX = $state(false);
  let moreY = $state(false);
  function watch() {
    if (!grid) {
      overflow = moreX = moreY = false;
      return;
    }
    const table = grid.querySelector('table');
    overflow = !!table && (table.offsetWidth > grid.offsetWidth - 2 || table.offsetHeight > grid.offsetHeight - 2);
    moreX = grid.scrollLeft + grid.clientWidth < grid.scrollWidth - 2;
    moreY = grid.scrollTop + grid.clientHeight < grid.scrollHeight - 2;
  }
  $effect(() => {
    void version;
    const el = grid;
    if (!el) return watch();
    const ro = new ResizeObserver(watch);
    ro.observe(el);
    const table = el.querySelector('table');
    if (table) ro.observe(table);
    return () => ro.disconnect();
  });
  onDestroy(() => (full = false));

  /** 点暗处：正在改单元格就先结束编辑，否则还原 */
  function onDim(e: PointerEvent) {
    e.stopPropagation();
    const a = document.activeElement as HTMLElement | null;
    if (a && body?.contains(a) && (a.isContentEditable || a.tagName === 'TEXTAREA')) {
      e.preventDefault();
      a.blur();
    } else full = false;
  }

  /** 单元格初次挂载时写入文字；之后由用户编辑，不再从数据重绘（避免光标跳动）。 */
  function initText(node: HTMLElement, text: string) {
    node.textContent = text;
  }

  function copyText() {
    const rows = commitSource();
    oncopy('text', rows, view === 'grid' ? toTsv(rows) : source);
  }

  function copyExcel() {
    const rows = commitSource();
    oncopy('excel', rows, toTsv(rows));
  }
</script>

{#if full}
  <div class="dim" role="presentation" transition:fade={{ duration: 160 }} onpointerdown={onDim} oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation(); }}></div>
{/if}
<div
  class="card"
  class:limit={cellWidth > 0}
  style:--cell={cellWidth}
  role="dialog"
  tabindex="-1"
  aria-label={t('表格')}
  use:floatCard={{
    sel,
    width: WIDTH,
    minWidth: 340,
    mode: full ? 'full' : 'normal',
    rev: `${version}|${cellWidth}`,
    onrestore: () => (full = false),
  }}
  onpointerdown={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.stopPropagation()}
>
  <header>
    <span class="chip">
      {#if card.stage === 'run'}<span class="dot" aria-hidden="true"></span>{t('识别表格中')}
      {:else if card.stage === 'model'}{t('需要下载表格模型')}
      {:else if card.stage === 'error'}{t('出错了')}
      {:else}{t('表格')}{/if}
    </span>
    <span class="grow"></span>
    {#if card.stage === 'done'}
      <Seg
        label={t('视图')}
        options={[
          { value: 'grid', label: t('表格') },
          { value: 'markdown', label: 'Markdown' },
          { value: 'csv', label: 'CSV' },
        ]}
        value={view}
        onchange={switchView}
      />
      {#if view === 'grid'}
        <IconButton
          icon="pen"
          label={mode === 'edit' ? t('正在编辑单元格（点一下回到预览）') : t('编辑单元格')}
          size={28}
          on={mode === 'edit'}
          onclick={() => onmode(mode === 'edit' ? 'preview' : 'edit')}
        />
      {/if}
    {/if}
    <IconButton icon="close" label={t('关闭 Esc')} size={28} onclick={onclose} />
  </header>

  {#if card.stage === 'run'}
    <div class="sk" aria-label={t('正在识别')}><span></span><span></span><span></span><span></span></div>
  {:else if card.stage === 'model'}
    <div class="model">
      <p>{t('本地表格识别要用 SLANet 表格结构模型（约 7.8 MB，来自 PaddlePaddle 官方仓库），识别模型没下载时一起下载，下载后离线可用。')}</p>
      {#if model?.downloading}
        <div class="bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)}>
          <span style:width="{pct}%"></span>
        </div>
        <span class="cap">{t('正在下载 {progressLabel}', { progressLabel: progressLabel(model) })}</span>
      {:else}
        {#if model?.error}<p class="err" role="alert">{model.error}</p>{/if}
        <div><Button variant="primary" onclick={ondownload}>{model?.error ? t('重新下载') : t('下载模型')}</Button></div>
      {/if}
    </div>
  {:else if card.stage === 'error'}
    <p class="err" role="alert">{card.error}</p>
  {:else}
    <div class="body" bind:this={body} class:more-x={moreX} class:more-y={moreY}>
    {#if view === 'grid'}
    <div class="grid" class:tight={overflow} class:editing={mode === 'edit'} data-grid bind:this={grid} onscroll={watch}>
      {#key version}
        <table>
          <tbody>
            {#each card.rows as row, r (r)}
              <tr>
                {#each row as cell, c (c)}
                  <td
                    class:th={cell.header}
                    style:--span={cell.colspan}
                    ondblclick={(e) => editOne(e, r, c)}
                    onblur={() => (single = '')}
                    colspan={cell.colspan > 1 ? cell.colspan : undefined}
                    rowspan={cell.rowspan > 1 ? cell.rowspan : undefined}
                    contenteditable={mode === 'edit' || single === `${r},${c}` ? 'plaintext-only' : 'false'}
                    spellcheck="false"
                    use:initText={cell.text}
                    oninput={(e) => editCell(r, c, e.currentTarget.innerText)}
                  ></td>
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      {/key}
    </div>
  {:else}
    <textarea class="field" aria-label={t('{v} 源码', { v: view === 'markdown' ? 'Markdown' : 'CSV' })} spellcheck="false" bind:value={source}></textarea>
    {/if}
    {#if full || overflow}
      <button
        type="button"
        class="corner"
        class:hint={!full}
        aria-label={full ? t('还原 Esc') : t('展开看全整张表格')}
        title={full ? t('还原 Esc') : t('展开看全整张表格')}
        onclick={() => (full = !full)}
      ><Icon name={full ? 'shrink' : 'expand'} size={14} stroke={2.2} /></button>
    {/if}
    </div>
  {/if}

  <footer>
    <span class="meta" title={card.meta}>{card.meta}</span>
    <Button disabled={!canAct} onclick={onexport} title={t('导出为 Excel 文件')}>{t('导出 .xlsx')}</Button>
    {#if view === 'grid'}
      <Button disabled={!canAct} onclick={copyText} title={t('复制为制表符分隔的纯文本')}>{t('复制纯文本')}</Button>
      <Button variant="primary" disabled={!canAct} onclick={copyExcel} title={t('复制后在 Excel / Word 里直接粘贴成表格，保留表头和合并单元格')}>{t('复制')}</Button>
    {:else}
      <Button variant="primary" disabled={!canAct} onclick={copyText} title={t('复制 {v} 源码', { v: view === 'csv' ? 'CSV' : 'Markdown' })}>{t('复制')}</Button>
    {/if}
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
  @keyframes card-in { from { opacity: 0; transform: translateY(calc(6px * var(--m))) scale(calc(1 - .03 * var(--m))); } }
  header, footer { display: flex; align-items: center; gap: 6px; }
  .grow { flex: 1; }
  .chip {
    display: inline-flex; align-items: center; gap: 6px;
    font-size: 12px; line-height: 22px; padding: 0 8px; border-radius: 6px;
    background: var(--sunken); color: var(--text2); font-weight: 500; white-space: nowrap;
  }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); animation: pulse 1s var(--ease-hover) infinite alternate; }
  @keyframes pulse { from { opacity: .35; transform: scale(calc(1 - .2 * var(--m))); } }
  .grid {
    max-height: 320px;
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line2);
    background: var(--field);
  }
  table { border-collapse: collapse; min-width: 100%; font-size: 13px; line-height: 19px; }
  td {
    border: 1px solid var(--line2);
    padding: 5px 9px;
    min-width: 48px;
    vertical-align: top;
    white-space: pre-wrap;
    outline: none;
    transition: background var(--d-hover) var(--ease-hover), box-shadow var(--d-hover) var(--ease-hover);
  }
  td.th { font-weight: 600; background: var(--sunken); }
  .editing td { cursor: text; }
  .editing td:hover { background: var(--hover); }
  td:focus { box-shadow: inset 0 0 0 2px var(--accent); background: var(--accent-soft); }
  .field {
    box-sizing: border-box; width: 100%; height: 240px; resize: vertical;
    border-radius: var(--r-ctl); border: 1px solid var(--line2); background: var(--field); color: var(--text);
    font: 12.5px/19px var(--font-mono, Consolas, monospace); padding: 8px 10px;
    transition: border-color var(--d-hover) var(--ease-hover), box-shadow var(--d-hover) var(--ease-hover);
  }
  .field, .sk { flex: 1 1 auto; min-height: 60px; }
  /* 拉伸过的卡片定高，表格 / 源码填满 */
  .card:global(.sized) .grid { max-height: none; }
  .body { position: relative; display: flex; flex-direction: column; flex: 1 1 auto; min-height: 0; }
  /* 右边 / 下边还有内容：边缘渐隐 */
  .body::before, .body::after { content: ''; position: absolute; z-index: 2; pointer-events: none; opacity: 0; transition: opacity var(--d-hover) var(--ease-hover); }
  .body::before { top: 1px; bottom: 13px; right: 13px; width: 24px; background: linear-gradient(to left, color-mix(in srgb, var(--text) 14%, transparent), transparent); }
  .body::after { left: 1px; right: 13px; bottom: 13px; height: 24px; background: linear-gradient(to top, color-mix(in srgb, var(--text) 14%, transparent), transparent); }
  .more-x::before, .more-y::after { opacity: 1; }
  .grid::-webkit-scrollbar { width: 12px; height: 12px; }
  .grid::-webkit-scrollbar-thumb { border: 3px solid transparent; border-radius: 6px; background: var(--text3) content-box; }
  .grid::-webkit-scrollbar-track { background: transparent; }
  /* 滚动条在角上的按钮前停住；内容也让开一点，滚到底时不会被按钮挡住 */
  .grid::-webkit-scrollbar-track:vertical { margin-bottom: 14px; }
  .grid::-webkit-scrollbar-track:horizontal { margin-right: 14px; }
  .grid::-webkit-scrollbar-corner { background: transparent; }
  .grid.tight { padding: 0 14px 14px 0; }
  .corner {
    position: absolute; right: 0; bottom: 0; z-index: 3;
    width: 26px; height: 26px; padding: 0; border: 1px solid var(--line2);
    border-radius: 8px 0 var(--r-ctl) 0;
    display: inline-flex; align-items: center; justify-content: center;
    background: var(--solid); color: var(--text2); cursor: pointer;
    transition: background-color var(--d-hover) var(--ease-hover), color var(--d-hover) var(--ease-hover), filter var(--d-hover) var(--ease-hover);
  }
  .corner.hint { background: var(--accent); border-color: var(--accent); color: var(--on-accent); }
  .corner:hover { filter: brightness(1.1); }
  .corner:active :global(svg) { transform: scale(.85); }
  .corner:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .dim { position: absolute; inset: 0; z-index: 45; background: var(--scrim); cursor: default; }
  .card:global(.full) { z-index: 46; }
  /* 展开：列宽按内容，单元格最宽 --cell（合并列按列数放宽），再长就换行 */
  .card:global(.full) .grid { max-height: none; padding: 0 0 28px 0; }
  .card:global(.full) table { width: max-content; font-size: var(--fs, 13px); line-height: 1.46; }
  .card:global(.full.limit) td { max-width: calc(var(--cell) * var(--span) * 1px); overflow-wrap: anywhere; }
  .card:global(.full) tr:first-child td { position: sticky; top: 0; z-index: 1; background: linear-gradient(var(--sunken), var(--sunken)), var(--solid); box-shadow: inset 0 -1px var(--line2); }
  .card:global(.full) tr:first-child td:focus { box-shadow: inset 0 0 0 2px var(--accent); }
  .card:global(.sized) .field { resize: none; }
  .field:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); }
  .sk { height: 180px; box-sizing: border-box; padding: 10px 2px; display: grid; grid-template-rows: repeat(4, 1fr); gap: 8px; }
  .sk span {
    border-radius: 6px;
    background: linear-gradient(90deg, var(--sunken) 0%, var(--hover) 50%, var(--sunken) 100%);
    background-size: 200% 100%; animation: shimmer 1.2s linear infinite;
  }
  @keyframes shimmer { from { background-position: 100% 0; } to { background-position: -100% 0; } }
  .model { display: flex; flex-direction: column; gap: 10px; font-size: 13px; line-height: 20px; color: var(--text2); }
  .model p { margin: 0; }
  .bar { height: 6px; border-radius: 3px; background: var(--sunken); overflow: hidden; }
  .bar span { display: block; height: 100%; border-radius: 3px; background: var(--accent); transition: width 200ms var(--ease-hover); }
  .cap { font-size: 11px; color: var(--text3); letter-spacing: .04em; }
  .err { margin: 0; font-size: 13px; line-height: 20px; color: var(--danger); white-space: pre-wrap; }
  .meta { flex: 1; min-width: 0; font-size: 12px; color: var(--text3); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
