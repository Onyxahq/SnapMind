<script lang="ts">
  // 贴图工具条（需求第 6 节）：所有贴图共用的一个窗口，由 Rust 摆到当前悬停的贴图下方（放不下时在上方）。
  // 顶部（或底部）留 8px 透明连接区，鼠标从贴图移过来不会丢；悬停 / 菜单打开期间一直显示。
  // 按钮只放已经能用的：复制、保存、更多、关闭；翻译、识别文字、Markdown、标注随对应任务加入。
  import { onMount } from 'svelte';
  import {
    onToolsTarget,
    sendPinAction,
    toolsHover,
    toolsLayout,
    toolsMenu,
    type PinAction,
    type ToolsTarget,
  } from '$lib/ipc';
  import { TOOLS_BRIDGE } from '$lib/pin';
  import IconButton from '$lib/ui/IconButton.svelte';
  import { popIn, popOut } from '$lib/motion';

  let target = $state<ToolsTarget | null>(null);
  let menuOpen = $state(false);
  let opacity = $state(100);
  let box: HTMLDivElement;

  const up = $derived(target?.placement === 'above');

  function act(action: PinAction, value?: number) {
    if (!target) return;
    sendPinAction(target.id, action, value);
    if (action !== 'opacity') setMenu(false);
  }

  function setMenu(open: boolean) {
    menuOpen = open;
    toolsMenu(open);
  }

  onMount(() => {
    const off = onToolsTarget((t) => {
      if (target?.id !== t.id) menuOpen = false;
      target = t;
      opacity = Math.round(t.opacity * 100);
    });
    // 内容尺寸变化（菜单展开 / 收起）时告诉 Rust 调整窗口大小
    const ro = new ResizeObserver(() => toolsLayout(box.offsetWidth, box.offsetHeight));
    ro.observe(box);
    return () => {
      off.then((f) => f());
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
  aria-label="贴图工具条"
  onpointerenter={() => toolsHover(true)}
  onpointerleave={() => toolsHover(false)}
>
  <div class="col" bind:this={box}>
    <div class="bar">
      <IconButton icon="copy" label="复制  Ctrl+C" onclick={() => act('copy')} />
      <IconButton icon="download" label="保存…  Ctrl+S" onclick={() => act('save')} />
      <IconButton icon="more" label="更多" on={menuOpen} onclick={() => setMenu(!menuOpen)} />
      <span class="sep" aria-hidden="true"></span>
      <IconButton icon="close" label="关闭  Esc" onclick={() => act('close')} />
    </div>
    {#if menuOpen}
      <div class="menu" role="menu" in:popIn out:popOut>
        <label class="row slider">
          <span>不透明度</span>
          <input
            type="range"
            min="10"
            max="100"
            step="10"
            bind:value={opacity}
            oninput={() => act('opacity', opacity / 100)}
            aria-label="不透明度"
          />
          <span class="num">{opacity}%</span>
        </label>
        <button type="button" class="row" role="menuitem" onclick={() => act('through')}>
          鼠标穿透<kbd>Ctrl+Shift+X</kbd>
        </button>
        <button type="button" class="row" role="menuitem" onclick={() => act('zoom100')}>
          缩放到 100%<kbd>Ctrl+0</kbd>
        </button>
      </div>
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
    from { opacity: 0; transform: translateY(-4px) scale(.96); }
  }
  .sep { width: 1px; height: 18px; margin: 0 4px; background: var(--line2); }
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
  .slider { cursor: default; color: var(--text2); }
  .slider:hover { background: transparent; }
  .slider input { flex: 1; accent-color: var(--accent); min-width: 0; }
  .num { width: 36px; text-align: right; font-variant-numeric: tabular-nums; font-size: 12px; }
</style>
