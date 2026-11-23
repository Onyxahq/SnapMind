<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 标注工具按钮 + 撤销（截图工具条和贴图工具条共用）。
  // 带样式的工具右侧有小箭头打开样式面板；再点一次当前工具也能开关面板。
  // onanchor 告诉宿主面板应对齐到哪个按钮（截图工具条用它摆放浮动面板）。
  import { READY_TOOLS, TOOL_INFO, TOOLS, type Tool } from '$lib/annotate';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Icon, { type IconName } from '$lib/ui/Icon.svelte';

  let {
    tools = READY_TOOLS,
    undo = true,
    tool,
    canUndo,
    propsOpen = $bindable(false),
    ontool,
    onundo,
    onanchor,
  }: {
    /** 显示哪些工具、按什么顺序（截图工具条按配置逐个放；默认全部） */
    tools?: Tool[];
    /** 是否在后面放撤销按钮 */
    undo?: boolean;
    tool: Tool | null;
    canUndo: boolean;
    propsOpen?: boolean;
    ontool: (t: Tool) => void;
    onundo: () => void;
    onanchor?: (offsetLeft: number) => void;
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

  function pick(t: Tool, el: HTMLElement) {
    if (tool === t) propsOpen = TOOL_INFO[t].props ? !propsOpen : false;
    else {
      ontool(t);
      propsOpen = false;
    }
    onanchor?.(el.offsetLeft);
  }

  function chevron(t: Tool, el: HTMLElement) {
    if (tool !== t) ontool(t);
    propsOpen = tool === t ? !propsOpen : true;
    onanchor?.((el.parentElement as HTMLElement).offsetLeft);
  }
</script>

{#each tools as tl (tl)}
  {@const info = TOOL_INFO[tl]}
  {@const key = TOOLS.indexOf(tl) + 1}
  <span class="tool" class:on={tool === tl}>
    <IconButton
      icon={ICON[tl]}
      label="{t(info.label)}  {key}"
      on={tool === tl}
      onclick={(e) => pick(tl, e.currentTarget.parentElement as HTMLElement)}
    />
    {#if info.props}
      <button
        type="button"
        class="chev"
        aria-label={t('{label}样式', { label: t(info.label) })}
        title={t('{label}样式', { label: t(info.label) })}
        onclick={(e) => chevron(tl, e.currentTarget)}
      ><Icon name="chevronDown" size={10} stroke={2.2} /></button>
    {/if}
  </span>
{/each}
{#if undo}
  <IconButton icon="undo" label={t('撤销 Ctrl+Z')} disabled={!canUndo} onclick={onundo} />
{/if}

<style>
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
</style>
