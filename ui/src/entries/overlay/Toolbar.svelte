<script lang="ts">
  // 截图工具条：标注工具 + 撤销 | 贴图、保存 | 退出、复制（需求 3.6，按钮 30px、内边距 4px）。
  // 带子类型 / 颜色 / 粗细的工具右侧有一个小箭头，点开属性弹层；再点一次当前工具也能开关弹层。
  import type { Rect } from '$lib/geometry';
  import { placeToolbar, type ToolbarAction as Action } from '$lib/toolbar';
  import { COLORS, READY_TOOLS, SIZES, TOOL_INFO, TOOLS, type Tool, type ToolOpts } from '$lib/annotate';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Icon, { type IconName } from '$lib/ui/Icon.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import { popIn, popOut } from '$lib/motion';

  let {
    sel,
    tool,
    opts,
    canUndo,
    propsOpen = $bindable(false),
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
    onaction: (a: Action) => void;
    ontool: (t: Tool) => void;
    onopts: (t: Tool, o: Partial<ToolOpts>) => void;
    onundo: () => void;
  } = $props();

  const ICON: Record<Tool, IconName> = {
    rect: 'rect',
    pen: 'pen',
    arrow: 'arrow',
    text: 'textT',
    number: 'number',
    mosaic: 'mosaic',
    eraser: 'eraser',
  };
  const tools = READY_TOOLS;

  let width = $state(420);
  let height = $state(38);
  let barEl = $state<HTMLDivElement>();
  const pos = $derived(placeToolbar(sel, { width, height }, { width: innerWidth, height: innerHeight }));
  /** 弹层朝下放不下时朝上开 */
  const popUp = $derived(pos.y + height + 150 > innerHeight);
  /** 弹层对齐当前工具按钮 */
  let popLeft = $state(4);

  function pick(t: Tool, el: HTMLElement) {
    if (tool === t) propsOpen = TOOL_INFO[t].props ? !propsOpen : false;
    else {
      ontool(t);
      propsOpen = false;
    }
    popLeft = el.offsetLeft;
  }

  function chevron(t: Tool, el: HTMLElement) {
    if (tool !== t) ontool(t);
    propsOpen = tool === t ? !propsOpen : true;
    popLeft = (el.parentElement as HTMLElement).offsetLeft;
  }

  const cur = $derived(tool && TOOL_INFO[tool].props ? opts[tool] : null);
  const subs = $derived(tool ? TOOL_INFO[tool].subs : null);
</script>

<div
  class="tb"
  role="toolbar"
  tabindex="-1"
  aria-label="截图工具条"
  style:left="{pos.x}px"
  style:top="{pos.y}px"
  bind:this={barEl}
  bind:clientWidth={width}
  bind:clientHeight={height}
  onpointerdown={(e) => e.stopPropagation()}
>
  {#each tools as t (t)}
    {@const info = TOOL_INFO[t]}
    {@const key = TOOLS.indexOf(t) + 1}
    <span class="tool" class:on={tool === t}>
      <IconButton
        icon={ICON[t]}
        label="{info.label}  {key}"
        on={tool === t}
        onclick={(e) => pick(t, e.currentTarget.parentElement as HTMLElement)}
      />
      {#if info.props}
        <button
          type="button"
          class="chev"
          aria-label="{info.label}样式"
          title="{info.label}样式"
          onclick={(e) => chevron(t, e.currentTarget)}
        ><Icon name="chevronDown" size={10} stroke={2.2} /></button>
      {/if}
    </span>
  {/each}
  <IconButton icon="undo" label="撤销  Ctrl+Z" disabled={!canUndo} onclick={onundo} />
  <span class="sep" aria-hidden="true"></span>
  <IconButton icon="pin" label="贴图  Ctrl+T" onclick={() => onaction('pin')} />
  <IconButton icon="download" label="保存…  Ctrl+S" onclick={() => onaction('save')} />
  <span class="sep" aria-hidden="true"></span>
  <IconButton icon="close" label="退出  Esc" onclick={() => onaction('exit')} />
  <button type="button" class="copy" title="复制  Ctrl+C / Enter" aria-label="复制" onclick={() => onaction('copy')}>
    <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
  </button>

  {#if propsOpen && tool && cur}
    <div
      class="props"
      class:up={popUp}
      style:left="{popLeft}px"
      role="dialog"
      aria-label="{TOOL_INFO[tool].label}样式"
      in:popIn
      out:popOut
    >
      {#if subs}
        <Seg
          label="子类型"
          options={subs.map((s, i) => ({ value: String(i), label: s }))}
          value={String(cur.sub)}
          onchange={(v) => onopts(tool!, { sub: Number(v) })}
        />
      {/if}
      {#if TOOL_INFO[tool].colors}
      <div class="colors">
        {#each COLORS as c, i (c.value)}
          <button
            type="button"
            class="cdot"
            class:on={cur.color === i}
            style:background={c.value}
            aria-label={c.label}
            title={c.label}
            onclick={() => onopts(tool!, { color: i })}
          ></button>
        {/each}
      </div>
      {/if}
      <div class="sizes">
        <span class="lbl">{tool === 'text' ? '字号' : tool === 'mosaic' ? '强度' : '粗细'}</span>
        {#each SIZES as s, i (s)}
          <button
            type="button"
            class="size"
            class:on={cur.size === i}
            aria-label="{s}px"
            title="{s}px"
            onclick={() => onopts(tool!, { size: i })}
          ><span style:width="{s * 2 + 2}px" style:height="{s * 2 + 2}px"></span></button>
        {/each}
      </div>
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
    from { opacity: 0; transform: translateY(4px) scale(.96); }
  }
  .tool {
    display: inline-flex;
    align-items: center;
    border-radius: var(--r-ctl);
    transition: background-color var(--d-hover) var(--ease-hover);
  }
  .tool.on { background: var(--accent-soft); }
  .chev {
    width: 12px;
    height: 30px;
    padding: 0;
    border: 0;
    margin-left: -2px;
    border-radius: 0 var(--r-ctl) var(--r-ctl) 0;
    background: transparent;
    color: var(--text3);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: color var(--d-hover), background-color var(--d-hover);
  }
  .chev:hover { color: var(--text); background: var(--hover); }
  .tool.on .chev { color: var(--accent); }
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
  .colors { display: flex; gap: 10px; align-items: center; }
  .cdot {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 0;
    padding: 0;
    cursor: pointer;
    box-shadow: inset 0 0 0 1px rgba(127, 127, 127, .35);
    transition: transform var(--d-hover) var(--ease-switch), box-shadow var(--d-hover);
  }
  .cdot:hover { transform: scale(1.12); }
  .cdot.on { box-shadow: inset 0 0 0 1px rgba(127, 127, 127, .35), 0 0 0 2px var(--glass2), 0 0 0 4px var(--accent); }
  .sizes { display: flex; gap: 4px; align-items: center; }
  .lbl { font-size: 12px; color: var(--text3); margin-right: 6px; }
  .size {
    width: 30px;
    height: 26px;
    border: 0;
    border-radius: var(--r-ctl);
    background: transparent;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: var(--text2);
    transition: background-color var(--d-hover);
  }
  .size:hover { background: var(--hover); }
  .size.on { background: var(--accent-soft); color: var(--accent); }
  .size span { border-radius: 50%; background: currentColor; }
</style>
