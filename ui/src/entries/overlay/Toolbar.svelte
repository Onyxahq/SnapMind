<script lang="ts">
  // 截图工具条（需求 3.6，按钮 30px、内边距 4px）：按钮的顺序、隐藏、收进「⋯ 更多」都读配置
  // （config.toolbar.capture，设置 → 工具栏）；退出、复制固定在最右边。标注工具和撤销之间、
  // 和动作按钮之间自动加分隔。标注工具按钮与样式面板和贴图工具条共用（$lib/annot）。
  import type { Rect } from '$lib/geometry';
  import { CAPTURE_ITEMS, asAction, placeToolbar, visibleBar, type BarItem, type ToolbarAction as Action } from '$lib/toolbar';
  import { config } from '$lib/config.svelte';
  import { openSettings } from '$lib/ipc';
  import { TOOL_INFO, type Tool, type ToolOpts } from '$lib/annotate';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import ToolButtons from '$lib/annot/ToolButtons.svelte';
  import ToolProps from '$lib/annot/ToolProps.svelte';
  import { popIn, popOut } from '$lib/motion';

  let {
    sel,
    tool,
    opts,
    canUndo,
    propsOpen = $bindable(false),
    rect = $bindable(),
    onaction,
    ontool,
    onopts,
    onundo,
  }: {
    sel: Rect;
    tool: Tool | null;
    opts: Record<Tool, ToolOpts>;
    canUndo: boolean;
    propsOpen?: boolean;
    /** 工具条当前占的位置（CSS 像素），结果卡片据此避开 */
    rect?: Rect;
    onaction: (a: Action) => void;
    ontool: (t: Tool) => void;
    onopts: (t: Tool, o: Partial<ToolOpts>) => void;
    onundo: () => void;
  } = $props();

  /** 「更多」菜单 */
  let moreOpen = $state(false);
  const bar = $derived(visibleBar(config.value.toolbar.capture, CAPTURE_ITEMS));
  /** 标注工具和撤销算一组，其余是动作：组和组之间加分隔 */
  const group = (i: BarItem) => (i.kind === 'action' ? 'action' : 'annot');
  const tip = (i: BarItem) => (i.key ? `${i.label}  ${i.key}` : i.label);

  function pickMore(i: BarItem) {
    moreOpen = false;
    if (i.kind === 'tool') ontool(i.id as Tool);
    else if (i.kind === 'undo') onundo();
    else onaction(asAction(i.id));
  }
  let moreAnchor = $state<HTMLSpanElement>();
  let width = $state(420);
  let height = $state(38);
  const pos = $derived(placeToolbar(sel, { width, height }, { width: innerWidth, height: innerHeight }));
  /** 弹层朝下放不下时朝上开 */
  const popUp = $derived(pos.y + height + 150 > innerHeight);
  /** 弹层对齐当前工具按钮 */
  let popLeft = $state(4);
  $effect(() => {
    rect = { x: pos.x, y: pos.y, width, height };
  });
</script>

<div
  class="tb"
  role="toolbar"
  tabindex="-1"
  aria-label="截图工具条"
  style:left="{pos.x}px"
  style:top="{pos.y}px"
  bind:clientWidth={width}
  bind:clientHeight={height}
  onpointerdown={(e) => e.stopPropagation()}
>
  {#each bar.main as it, i (it.id)}
    {#if i > 0 && group(bar.main[i - 1]) !== group(it)}<span class="sep" aria-hidden="true"></span>{/if}
    {#if it.kind === 'tool'}
      <ToolButtons tools={[it.id as Tool]} undo={false} {tool} {canUndo} bind:propsOpen {ontool} {onundo} onanchor={(x) => (popLeft = x)} />
    {:else if it.kind === 'undo'}
      <IconButton icon="undo" label={tip(it)} disabled={!canUndo} onclick={onundo} />
    {:else}
      <IconButton icon={it.icon} label={tip(it)} onclick={() => onaction(asAction(it.id))} />
    {/if}
  {/each}
  {#if bar.main.length}<span class="sep" aria-hidden="true"></span>{/if}
  <span class="anchor" bind:this={moreAnchor}>
    <IconButton icon="more" label="更多" on={moreOpen} onclick={() => (moreOpen = !moreOpen)} />
  </span>
  <span class="sep" aria-hidden="true"></span>
  <IconButton icon="close" label="退出  Esc" onclick={() => onaction('exit')} />
  <button type="button" class="copy" title="复制  Ctrl+C / Enter" aria-label="复制" onclick={() => onaction('copy')}>
    <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
  </button>

  {#if moreOpen}
    <div class="more" class:up={popUp} style:left="{moreAnchor?.offsetLeft ?? 0}px" role="menu" aria-label="更多" in:popIn out:popOut>
      {#each bar.more as m (m.id)}
        <button type="button" role="menuitem" disabled={m.kind === 'undo' && !canUndo} onclick={() => pickMore(m)}>
          <Icon name={m.icon} size={16} /><span class="ml">{m.label}</span>{#if m.key}<kbd>{m.key}</kbd>{/if}
        </button>
      {/each}
      {#if bar.more.length}<span class="msep" aria-hidden="true"></span>{/if}
      <button
        type="button"
        role="menuitem"
        class="custom"
        onclick={() => {
          moreOpen = false;
          openSettings('bar');
        }}
      >
        <span class="ml">自定义工具条…</span>
      </button>
    </div>
  {/if}
  {#if propsOpen && tool && TOOL_INFO[tool].props}
    <div
      class="props"
      class:up={popUp}
      style:left="{popLeft}px"
      role="dialog"
      aria-label="{TOOL_INFO[tool].label}样式"
      in:popIn
      out:popOut
    >
      <ToolProps {tool} opts={opts[tool]} {onopts} />
    </div>
  {/if}
</div>

<style>
  .tb {
    position: absolute;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px;
    border-radius: var(--r-panel);
    background: var(--glass2);
    backdrop-filter: var(--blur-1);
    box-shadow: var(--elev-sm);
    color: var(--text);
    z-index: 30;
    animation: tb-in var(--d-in) var(--ease-in) both;
  }
  @keyframes tb-in {
    from { opacity: 0; transform: translateY(calc(4px * var(--m))) scale(calc(1 - .04 * var(--m))); }
  }
  .sep {
    width: 1px;
    height: 18px;
    margin: 0 4px;
    background: var(--line2);
  }
  .copy {
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: var(--r-ctl);
    background: var(--accent);
    color: var(--on-accent);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition:
      filter var(--d-hover) var(--ease-hover),
      transform var(--d-press) var(--ease-press);
  }
  .copy:hover { filter: brightness(1.08); }
  .copy:active { transform: scale(.9); }
  .copy svg {
    fill: none;
    stroke: currentColor;
    stroke-width: 2.2;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .anchor { display: inline-flex; }
  .more {
    position: absolute;
    top: calc(100% + 6px);
    display: flex;
    flex-direction: column;
    padding: 4px;
    border-radius: var(--r-panel);
    background: var(--glass2);
    backdrop-filter: var(--blur-2);
    box-shadow: var(--elev);
    min-width: 180px;
  }
  .more.up { top: auto; bottom: calc(100% + 6px); }
  .more button {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: var(--r-ctl);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    cursor: pointer;
    transition: background var(--d-hover) var(--ease-hover);
  }
  .more button:hover { background: var(--hover); }
  .more button:disabled { opacity: .45; cursor: default; }
  .more .ml { flex: 1; text-align: left; }
  .more kbd { font: 11px/18px var(--font-mono, Consolas, monospace); color: var(--text3); }
  .more .custom { color: var(--text2); }
  .msep { height: 1px; margin: 4px 6px; background: var(--line2); }
  .props {
    position: absolute;
    top: calc(100% + 6px);
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    border-radius: var(--r-panel);
    background: var(--glass2);
    backdrop-filter: var(--blur-2);
    box-shadow: var(--elev);
    min-width: 200px;
  }
  .props.up { top: auto; bottom: calc(100% + 6px); }
</style>
