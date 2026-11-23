<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 贴图工具条（需求第 6 节）：所有贴图共用的一个窗口，由 Rust 摆到当前悬停的贴图下方（放不下时在上方）。
  // 顶部（或底部）留 8px 透明连接区，鼠标从贴图移过来不会丢；悬停 / 菜单打开 / 标注期间一直显示。
  // 普通模式：按钮的顺序、隐藏、收进「更多」读配置（config.toolbar.pin，设置 → 工具栏）；
  // 关闭固定在最右边，不透明度滑块固定在「更多」里。
  // 标注模式（M3.3）：与截图相同的标注工具（共用组件）+ 撤销 | 复制、完成；样式面板排在工具条旁边。
  import { onMount } from 'svelte';
  import {
    openSettings,
    onAnnotState,
    onToolsTarget,
    sendPinAction,
    toolsHover,
    toolsLayout,
    toolsMenu,
    type PinActionMsg,
    type PinAnnotState,
    type ToolsTarget,
  } from '$lib/ipc';
  import { TOOLS_BRIDGE } from '$lib/pin';
  import { TOOL_INFO } from '$lib/annotate';
  import IconButton from '$lib/ui/IconButton.svelte';
  import ToolButtons from '$lib/annot/ToolButtons.svelte';
  import ToolProps from '$lib/annot/ToolProps.svelte';
  import { popIn, popOut } from '$lib/motion';
  import { config } from '$lib/config.svelte';
  import { PIN_ITEMS, visibleBar, type BarItem } from '$lib/toolbar';
  import Icon from '$lib/ui/Icon.svelte';

  const bar = $derived(visibleBar(config.value.toolbar.pin, PIN_ITEMS));
  const tip = (i: BarItem) => (i.key ? `${t(i.label)}  ${t(i.key)}` : t(i.label));

  let target = $state<ToolsTarget | null>(null);
  /** 各贴图的标注状态（贴图主动推送） */
  let annots = $state<Record<number, PinAnnotState>>({});
  let menuOpen = $state(false);
  let propsOpen = $state(false);
  let opacity = $state(100);
  let box: HTMLDivElement;

  const up = $derived(target?.placement === 'above');
  const annot = $derived(target ? annots[target.id] : undefined);
  const annotating = $derived(annot?.active ?? false);

  function send(msg: PinActionMsg) {
    if (!target) return;
    sendPinAction(target.id, msg);
  }

  function act(action: PinActionMsg['action'], value?: number) {
    send({ action, value });
    if (action !== 'opacity') setMenu(false);
  }

  function setMenu(open: boolean) {
    menuOpen = open;
    toolsMenu(open);
  }

  onMount(() => {
    const offTarget = onToolsTarget((t) => {
      if (target?.id !== t.id) {
        menuOpen = false;
        propsOpen = false;
      }
      target = t;
      opacity = Math.round(t.opacity * 100);
    });
    const offAnnot = onAnnotState((s) => {
      annots = { ...annots, [s.id]: s };
      if (!s.active && target?.id === s.id) propsOpen = false;
    });
    // 内容尺寸变化（菜单 / 样式面板展开收起、切换标注模式）时告诉 Rust 调整窗口大小
    const ro = new ResizeObserver(() => toolsLayout(box.offsetWidth, box.offsetHeight));
    ro.observe(box);
    return () => {
      offTarget.then((f) => f());
      offAnnot.then((f) => f());
      ro.disconnect();
    };
  });
</script>

<div
  class="wrap"
  class:up
  style:padding-top="{up ? 0 : TOOLS_BRIDGE}px"
  style:padding-bottom="{up ? TOOLS_BRIDGE : 0}px"
  role="toolbar"
  tabindex="-1"
  aria-label={t('贴图工具条')}
  onpointerenter={() => toolsHover(true)}
  onpointerleave={() => toolsHover(false)}
>
  <div class="col" bind:this={box}>
    {#if annotating && annot}
      <div class="bar">
        <ToolButtons
          tool={annot.tool}
          canUndo={annot.canUndo}
          bind:propsOpen
          ontool={(t) => send({ action: 'tool', tool: t })}
          onundo={() => send({ action: 'undo' })}
        />
        <span class="sep" aria-hidden="true"></span>
        <IconButton icon="copy" label={t('复制 Ctrl+C')} onclick={() => act('copy')} />
        <button type="button" class="done" title={t('完成 空格')} onclick={() => act('done')}>{t('完成')}</button>
      </div>
      {#if propsOpen && annot.tool && TOOL_INFO[annot.tool].props}
        <div class="panel" in:popIn out:popOut>
          <ToolProps
            tool={annot.tool}
            opts={annot.opts[annot.tool]}
            onopts={(t, o) => send({ action: 'opts', tool: t, opts: o })}
          />
        </div>
      {/if}
    {:else}
      <div class="bar">
        {#each bar.main as it (it.id)}
          <IconButton icon={it.icon} label={tip(it)} onclick={() => act(it.id as PinActionMsg['action'])} />
        {/each}
        <IconButton icon="more" label={t('更多')} on={menuOpen} onclick={() => setMenu(!menuOpen)} />
        <span class="sep" aria-hidden="true"></span>
        <IconButton icon="close" label={t('关闭 Esc')} onclick={() => act('close')} />
      </div>
      {#if menuOpen}
        <div class="menu" role="menu" in:popIn out:popOut>
          <label class="row slider">
            <span>{t('不透明度')}</span>
            <input
              type="range"
              min="10"
              max="100"
              step="10"
              bind:value={opacity}
              oninput={() => act('opacity', opacity / 100)}
              aria-label={t('不透明度')}
            />
            <span class="num">{opacity}%</span>
          </label>
          {#each bar.more as it (it.id)}
            <button type="button" class="row" role="menuitem" onclick={() => act(it.id as PinActionMsg['action'])}>
              <Icon name={it.icon} size={15} /><span class="ml">{t(it.label)}</span>{#if it.key}<kbd>{t(it.key)}</kbd>{/if}
            </button>
          {/each}
          <span class="msep" aria-hidden="true"></span>
          <button
            type="button"
            class="row dim"
            role="menuitem"
            onclick={() => {
              setMenu(false);
              openSettings('bar');
            }}
          >
            <span class="ml">{t('自定义工具条…')}</span>
          </button>
        </div>
      {/if}
    {/if}
  </div>
</div>

<style>
  :global(html), :global(body) {
    background: transparent;
    overflow: hidden;
    margin: 0;
    user-select: none;
  }
  .wrap {
    display: inline-block;
  }
  .col {
    display: inline-flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    /* 留出阴影的位置 */
    padding: 2px 4px 8px;
  }
  .up .col { flex-direction: column-reverse; padding: 8px 4px 2px; }
  .bar {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px;
    border-radius: var(--r-panel);
    background: var(--glass2);
    box-shadow: var(--elev-sm);
    color: var(--text);
    animation: bar-in var(--d-in) var(--ease-in) both;
  }
  @keyframes bar-in {
    from { opacity: 0; transform: translateY(calc(-4px * var(--m))) scale(calc(1 - .04 * var(--m))); }
  }
  .sep { width: 1px; height: 18px; margin: 0 4px; background: var(--line2); }
  .done {
    height: 30px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--r-ctl);
    background: var(--accent);
    color: var(--on-accent);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: filter var(--d-hover) var(--ease-hover), transform var(--d-press) var(--ease-press);
  }
  .done:hover { filter: brightness(1.08); }
  .done:active { transform: scale(.95); }
  .panel {
    padding: 12px;
    border-radius: var(--r-panel);
    background: var(--glass2);
    box-shadow: var(--elev);
    min-width: 200px;
  }
  .menu {
    min-width: 230px;
    padding: 6px;
    border-radius: var(--r-panel);
    background: var(--glass2);
    box-shadow: var(--elev);
    color: var(--text);
    display: flex;
    flex-direction: column;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
    text-align: left;
    transition: background-color var(--d-hover) var(--ease-hover);
  }
  .row:hover { background: var(--hover); }
  .row kbd { margin-left: auto; font: inherit; font-size: 11px; color: var(--text3); }
  .row .ml { flex: 1; }
  .row.dim { color: var(--text2); }
  .msep { height: 1px; margin: 4px 6px; background: var(--line2); }
  .slider { cursor: default; color: var(--text2); }
  .slider:hover { background: transparent; }
  .slider input { flex: 1; accent-color: var(--accent); min-width: 0; }
  .num { width: 36px; text-align: right; font-variant-numeric: tabular-nums; font-size: 12px; }
</style>
