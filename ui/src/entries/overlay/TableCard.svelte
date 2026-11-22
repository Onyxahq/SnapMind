<script lang="ts">
  // 表格结果卡片（需求 3.8、M8.2）：「表格」视图里单元格直接改；「Markdown」「HTML」视图里改源码，
  // 切回表格时解析回来。复制（按当前视图：表格复制成 TSV，粘贴进 Excel 自动分格）和导出 .xlsx 会结束截图。
  import type { Rect } from '$lib/geometry';
  import type { OcrStatus } from '$lib/ipc';
  import { placeCard } from '$lib/task';
  import { progressLabel } from '$lib/ocr';
  import { parseHtml, parseMarkdown, toHtml, toMarkdown, type TableCardState, type TableView, type TRows } from '$lib/table';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Seg from '$lib/ui/Seg.svelte';

  let {
    sel,
    card,
    model,
    onrows,
    ondownload,
    oncopy,
    onexport,
    onclose,
  }: {
    sel: Rect;
    card: TableCardState;
    model: OcrStatus | null;
    onrows: (rows: TRows) => void;
    ondownload: () => void;
    /** 复制当前视图的内容 */
    oncopy: (view: TableView, source: string) => void;
    onexport: () => void;
    onclose: () => void;
  } = $props();

  let height = $state(320);
  const WIDTH = 460;
  const pos = $derived(placeCard(sel, { width: WIDTH, height }, { width: innerWidth, height: innerHeight }));
  const canAct = $derived(card.stage === 'done' && card.rows.length > 0);
  const pct = $derived(model && model.total ? Math.min(100, (model.received / model.total) * 100) : 0);

  let view = $state<TableView>('grid');
  /** 源码视图里正在编辑的文字，以及刚生成时的样子（没改过就不解析回去——Markdown 表示不了合并单元格） */
  let source = $state('');
  let generated = '';
  /** 换了一份数据就重建表格（单元格是 contenteditable，编辑时不重绘） */
  let version = $state(0);

  function switchView(next: TableView) {
    // 离开源码视图：把改过的源码解析回表格（没改过、或解析不出来就保留原来的）
    if (view !== 'grid' && source !== generated) {
      const parsed = view === 'markdown' ? parseMarkdown(source) : parseHtml(source);
      if (parsed) {
        onrows(parsed);
        version++;
      }
    }
    if (next !== 'grid') source = generated = next === 'markdown' ? toMarkdown(card.rows) : toHtml(card.rows);
    view = next;
  }

  function editCell(r: number, c: number, text: string) {
    const rows = card.rows.map((row) => row.slice());
    rows[r][c] = { ...rows[r][c], text };
    onrows(rows);
  }

  /** 单元格初次挂载时写入文字；之后由用户编辑，不再从数据重绘（避免光标跳动）。 */
  function initText(node: HTMLElement, text: string) {
    node.textContent = text;
  }

  function copy() {
    oncopy(view, source);
  }
</script>

<div
  class="card"
  role="dialog"
  tabindex="-1"
  aria-label="表格"
  style:left="{pos.x}px"
  style:top="{pos.y}px"
  style:width="{WIDTH}px"
  bind:clientHeight={height}
  onpointerdown={(e) => e.stopPropagation()}
  oncontextmenu={(e) => e.stopPropagation()}
>
  <header>
    <span class="chip">
      {#if card.stage === 'run'}<span class="dot" aria-hidden="true"></span>识别表格中
      {:else if card.stage === 'model'}需要下载表格模型
      {:else if card.stage === 'error'}出错了
      {:else}表格{/if}
    </span>
    <span class="grow"></span>
    {#if card.stage === 'done'}
      <Seg
        label="视图"
        options={[
          { value: 'grid', label: '表格' },
          { value: 'markdown', label: 'Markdown' },
          { value: 'html', label: 'HTML' },
        ]}
        value={view}
        onchange={switchView}
      />
    {/if}
    <IconButton icon="close" label="关闭  Esc" size={28} onclick={onclose} />
  </header>

  {#if card.stage === 'run'}
    <div class="sk" aria-label="正在识别"><span></span><span></span><span></span><span></span></div>
  {:else if card.stage === 'model'}
    <div class="model">
      <p>本地表格识别要用 SLANet 表格结构模型（约 7.8 MB，来自 PaddlePaddle 官方仓库），识别模型没下载时一起下载，下载后离线可用。</p>
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
  {:else if view === 'grid'}
    <div class="grid">
      {#key version}
        <table>
          <tbody>
            {#each card.rows as row, r (r)}
              <tr>
                {#each row as cell, c (c)}
                  <td
                    class:th={cell.header}
                    colspan={cell.colspan > 1 ? cell.colspan : undefined}
                    rowspan={cell.rowspan > 1 ? cell.rowspan : undefined}
                    contenteditable="plaintext-only"
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
    <textarea class="field" aria-label="{view === 'markdown' ? 'Markdown' : 'HTML'} 源码" spellcheck="false" bind:value={source}></textarea>
  {/if}

  <footer>
    <span class="meta" title={card.meta}>{card.meta}</span>
    <Button disabled={!canAct} onclick={onexport} title="导出为 Excel 文件">导出 .xlsx</Button>
    <Button variant="primary" disabled={!canAct} onclick={copy} title={view === 'grid' ? '复制为制表符分隔（粘贴进 Excel 自动分格）' : '复制源码'}>复制</Button>
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
  }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); animation: pulse 1s var(--ease-hover) infinite alternate; }
  @keyframes pulse { from { opacity: .35; transform: scale(.8); } }
  .grid {
    max-height: 320px;
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
  td:hover { background: var(--hover); }
  td:focus { box-shadow: inset 0 0 0 2px var(--accent); background: var(--accent-soft); }
  .field {
    box-sizing: border-box; width: 100%; height: 240px; resize: vertical;
    border-radius: var(--r-ctl); border: 1px solid var(--line2); background: var(--field); color: var(--text);
    font: 12.5px/19px var(--font-mono, Consolas, monospace); padding: 8px 10px;
    transition: border-color var(--d-hover) var(--ease-hover), box-shadow var(--d-hover) var(--ease-hover);
  }
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
