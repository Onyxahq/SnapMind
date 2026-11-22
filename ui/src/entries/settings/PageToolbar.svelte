<script lang="ts">
  // 设置 → 工具栏（需求 3.6、第 9 节、M10.2）：截图工具条和贴图工具条各一份布局。
  // 拖动左边的把手排序（指针跟手，其他行让位有动画），取消勾选即隐藏，「更多」分隔线以下的按钮收进「⋯」菜单。
  // 每次改动立即保存，已经打开的工具条同时刷新。退出 / 复制（截图）和关闭、不透明度（贴图）固定，不在列表里。
  import { flip } from '$lib/motion';
  import { DEFAULT_CONFIG } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import {
    CAPTURE_ITEMS,
    PIN_ITEMS,
    layoutRows,
    toLayout,
    visibleBar,
    type BarItem,
    type LayoutRow,
  } from '$lib/toolbar';
  import { toast } from '$lib/ui/toast.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import Head from './Head.svelte';

  type Which = 'capture' | 'pin';
  let which = $state<Which>('capture');
  const catalog = $derived(which === 'capture' ? CAPTURE_ITEMS : PIN_ITEMS);

  /** 正在编辑的一列（拖动时只改它，松手才保存） */
  let rows = $state<LayoutRow[]>([]);
  let drag = $state<{ key: string; startY: number; offset: number; h: number } | null>(null);
  $effect(() => {
    const layout = config.value.toolbar[which];
    if (!drag) rows = layoutRows(layout, catalog);
  });

  const keyOf = (r: LayoutRow) => ('divider' in r ? '__more' : r.item.id);
  const bar = $derived(visibleBar(toLayout(rows), catalog));
  const group = (i: BarItem) => (i.kind === 'action' ? 'action' : 'annot');

  async function commit(next: LayoutRow[] = rows) {
    try {
      await updateConfig({ toolbar: { [which]: toLayout(next) } });
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  function move(from: number, to: number) {
    if (to < 0 || to >= rows.length || from === to) return;
    const next = rows.slice();
    const [r] = next.splice(from, 1);
    next.splice(to, 0, r);
    rows = next;
  }

  function toggle(i: number) {
    const r = rows[i];
    if ('divider' in r) return;
    rows = rows.map((x, k) => (k === i ? { ...r, hidden: !r.hidden } : x));
    commit();
  }

  function step(i: number, d: number) {
    move(i, i + d);
    commit();
  }

  async function reset() {
    try {
      await updateConfig({ toolbar: { [which]: structuredClone(DEFAULT_CONFIG.toolbar[which]) } });
      toast(which === 'capture' ? '截图工具条已恢复默认' : '贴图工具条已恢复默认');
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  // ---- 拖动排序：把手按下后指针移动超过半行就和邻居换位 ----
  function down(e: PointerEvent, r: LayoutRow) {
    if (e.button !== 0) return;
    const row = (e.currentTarget as HTMLElement).closest('.trow') as HTMLElement;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { key: keyOf(r), startY: e.clientY, offset: 0, h: row.offsetHeight };
    e.preventDefault();
  }

  function moveDrag(e: PointerEvent) {
    if (!drag) return;
    let offset = e.clientY - drag.startY;
    let startY = drag.startY;
    let i = rows.findIndex((r) => keyOf(r) === drag!.key);
    while (offset > drag.h / 2 && i < rows.length - 1) {
      move(i, i + 1);
      i++;
      offset -= drag.h;
      startY += drag.h;
    }
    while (offset < -drag.h / 2 && i > 0) {
      move(i, i - 1);
      i--;
      offset += drag.h;
      startY -= drag.h;
    }
    drag = { ...drag, offset, startY };
  }

  function up() {
    if (!drag) return;
    drag = null;
    commit();
  }
</script>

<Head title="工具栏" desc="拖动排序，取消勾选即隐藏。「更多」分隔线以下的按钮会收进「⋯」菜单。改动立即生效。">
  <Button variant="ghost" onclick={reset}>恢复默认</Button>
</Head>

<div class="seg">
  <Seg
    label="工具条"
    options={[
      { value: 'capture', label: '截图工具条' },
      { value: 'pin', label: '贴图工具条' },
    ]}
    value={which}
    onchange={(v) => (which = v)}
  />
</div>

<div class="prev-wrap">
  <span class="cap">预览</span>
  <div class="prev" aria-label="工具条预览">
    {#each bar.main as it, i (it.id)}
      {#if which === 'capture' && i > 0 && group(bar.main[i - 1]) !== group(it)}<span class="sep"></span>{/if}
      <span class="i" title={it.label}><Icon name={it.icon} size={16} /></span>
    {/each}
    {#if which === 'capture' && bar.main.length}<span class="sep"></span>{/if}
    <span class="i" title="更多"><Icon name="more" size={16} /></span>
    <span class="sep"></span>
    <span class="i" title={which === 'capture' ? '退出' : '关闭'}><Icon name="close" size={16} /></span>
    {#if which === 'capture'}<span class="i ok" title="复制"><Icon name="check" size={16} /></span>{/if}
  </div>
  <span class="cap">
    「⋯」里：{[...bar.more.map((m) => m.label), ...(which === 'pin' ? ['不透明度'] : []), '自定义工具条…'].join('、')}
  </span>
</div>

<div class="card" role="list" aria-label="按钮列表">
  {#each rows as r, i (keyOf(r))}
    {@const dragging = drag?.key === keyOf(r)}
    <div
      class="trow"
      class:div={'divider' in r}
      class:dragging
      class:off={!('divider' in r) && r.hidden}
      role="listitem"
      style:transform={dragging ? `translateY(${drag?.offset ?? 0}px)` : undefined}
      animate:flip={{ duration: dragging ? 0 : 160 }}
    >
      <button
        type="button"
        class="grip"
        aria-label="拖动排序"
        onpointerdown={(e) => down(e, r)}
        onpointermove={moveDrag}
        onpointerup={up}
        onpointercancel={up}
      >
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 6h.01M15 6h.01M9 12h.01M15 12h.01M9 18h.01M15 18h.01" /></svg>
      </button>
      {#if 'divider' in r}
        <span class="dl">—— 以下收进「⋯ 更多」——</span>
      {:else}
        <input type="checkbox" checked={!r.hidden} aria-label="显示 {r.item.label}" onchange={() => toggle(i)} />
        <span class="ic"><Icon name={r.item.icon} size={15} /></span>
        <span class="lbl">{r.item.label}</span>
        {#if r.item.key}<span class="k">{r.item.key}</span>{/if}
      {/if}
      <IconButton icon="chevronUp" label="上移" size={26} disabled={i === 0} onclick={() => step(i, -1)} />
      <IconButton icon="chevronDown" label="下移" size={26} disabled={i === rows.length - 1} onclick={() => step(i, 1)} />
    </div>
  {/each}
</div>

<style>
  .seg { align-self: flex-start; }
  .prev-wrap { display: flex; flex-direction: column; gap: 8px; align-items: flex-start; }
  .cap { font-size: 12px; color: var(--text3); }
  .prev {
    display: flex; align-items: center; gap: 1px; padding: 4px; min-height: 28px;
    border-radius: var(--r-panel); background: var(--glass2); box-shadow: var(--elev-sm);
  }
  .i { width: 28px; height: 28px; border-radius: 7px; display: inline-flex; align-items: center; justify-content: center; color: var(--text2); animation: pop 160ms var(--ease-in) both; }
  @keyframes pop { from { opacity: 0; transform: scale(calc(1 - .2 * var(--m))); } }
  .i.ok { background: var(--accent); color: var(--on-accent, #fff); margin-left: 2px; }
  .sep { width: 1px; height: 16px; margin: 0 4px; background: var(--line2); }
  .card { border-radius: var(--r-panel); background: var(--card); box-shadow: var(--card-ring); }
  .trow {
    position: relative;
    display: flex; align-items: center; gap: 10px;
    height: 44px; box-sizing: border-box; padding: 0 10px 0 6px;
    background: var(--card);
    transition: box-shadow 120ms var(--ease-hover), opacity 120ms var(--ease-hover);
  }
  .trow:first-child { border-radius: var(--r-panel) var(--r-panel) 0 0; }
  .trow:last-child { border-radius: 0 0 var(--r-panel) var(--r-panel); }
  .trow + .trow { border-top: 1px solid var(--line); }
  .trow.dragging { z-index: 2; box-shadow: var(--elev); border-radius: 8px; cursor: grabbing; }
  .trow.div { background: var(--sunken); }
  .trow.off .ic, .trow.off .lbl, .trow.off .k { opacity: .45; }
  .grip {
    width: 24px; height: 32px; padding: 0; border: 0; border-radius: 6px; background: transparent;
    color: var(--text3); cursor: grab; touch-action: none; display: inline-flex; align-items: center; justify-content: center;
  }
  .grip:hover { background: var(--hover); color: var(--text2); }
  .grip svg { width: 16px; height: 16px; fill: none; stroke: currentColor; stroke-width: 2.6; stroke-linecap: round; }
  input[type='checkbox'] { width: 16px; height: 16px; accent-color: var(--accent); margin: 0; }
  .ic {
    width: 28px; height: 28px; border-radius: 7px; background: var(--sunken); color: var(--text2);
    display: inline-flex; align-items: center; justify-content: center; transition: opacity 120ms;
  }
  .lbl { flex: 1; font-size: 13px; transition: opacity 120ms; }
  .dl { flex: 1; font-size: 12px; font-weight: 600; color: var(--text2); }
  .k { font: 12px/20px var(--font-mono, Consolas, monospace); color: var(--text2); padding: 0 8px; border-radius: 6px; background: var(--sunken); transition: opacity 120ms; }
</style>
