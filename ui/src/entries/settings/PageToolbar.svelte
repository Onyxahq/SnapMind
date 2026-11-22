<script lang="ts">
  // 设置 → 工具栏：截图工具条上现有的按钮和顺序（预览 + 列表）。拖动排序、隐藏、收进「更多」在 M10.2 加入。
  import { READY_TOOLS, TOOLS, TOOL_INFO, type Tool } from '$lib/annotate';
  import Icon, { type IconName } from '$lib/ui/Icon.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Head from './Head.svelte';

  const TOOL_ICON: Record<Tool, IconName> = {
    rect: 'rect',
    pen: 'pen',
    arrow: 'arrow',
    text: 'textT',
    number: 'number',
    mosaic: 'mosaic',
    eraser: 'eraser',
  };

  type Item = { icon: IconName; label: string; key: string };
  const tools: Item[] = [
    ...READY_TOOLS.map((t) => ({
      icon: TOOL_ICON[t],
      label: TOOL_INFO[t].subs ? TOOL_INFO[t].subs!.join(' / ') : TOOL_INFO[t].label,
      key: String(TOOLS.indexOf(t) + 1),
    })),
    { icon: 'undo', label: '撤销', key: 'Ctrl+Z' },
  ];
  const actions: Item[] = [
    { icon: 'long', label: '长截图', key: 'L' },
    { icon: 'video', label: '录屏', key: 'R' },
    { icon: 'scan', label: '识别文字', key: 'Shift+C' },
    { icon: 'markdown', label: '识别为 Markdown', key: 'Shift+Alt+R' },
    { icon: 'translate', label: '翻译', key: 'Ctrl+Q' },
    { icon: 'pin', label: '贴图', key: 'Ctrl+T' },
    { icon: 'download', label: '保存', key: 'Ctrl+S' },
  ];
  const more: Item[] = [
    { icon: 'formula', label: '公式识别', key: '' },
    { icon: 'table', label: '表格识别', key: '' },
  ];
  const groups: { title: string; items: Item[] }[] = [
    { title: '标注', items: tools },
    { title: '动作', items: actions },
    { title: '收在「⋯ 更多」里', items: more },
  ];
</script>

<Head title="工具栏" desc="截图工具条上的按钮和顺序。标注工具的子类型、颜色和粗细在按钮旁的下拉里切换。" />

<div class="prev-wrap">
  <span class="cap">预览</span>
  <div class="prev" aria-label="截图工具条预览">
    {#each tools as t (t.label)}<span class="i" title={t.label}><Icon name={t.icon} size={16} /></span>{/each}
    <span class="sep"></span>
    {#each actions.slice(0, 5) as t (t.label)}<span class="i" title={t.label}><Icon name={t.icon} size={16} /></span>{/each}
    <span class="sep"></span>
    <span class="i" title="更多"><Icon name="more" size={16} /></span>
    <span class="sep"></span>
    {#each actions.slice(5) as t (t.label)}<span class="i" title={t.label}><Icon name={t.icon} size={16} /></span>{/each}
    <span class="sep"></span>
    <span class="i" title="退出"><Icon name="close" size={16} /></span>
    <span class="i ok" title="复制"><Icon name="check" size={16} /></span>
  </div>
</div>

{#each groups as g (g.title)}
  <Card title={g.title}>
    {#each g.items as it (it.label)}
      <div class="trow">
        <span class="ic"><Icon name={it.icon} size={15} /></span>
        <span class="lbl">{it.label}</span>
        {#if it.key}<span class="k">{it.key}</span>{/if}
      </div>
    {/each}
  </Card>
{/each}

<style>
  .prev-wrap { display: flex; flex-direction: column; gap: 8px; align-items: flex-start; }
  .cap { font-size: 12px; color: var(--text3); }
  .prev {
    display: flex; align-items: center; gap: 1px; padding: 4px;
    border-radius: var(--r-panel); background: var(--glass2); box-shadow: var(--elev-sm);
  }
  .i { width: 28px; height: 28px; border-radius: 7px; display: inline-flex; align-items: center; justify-content: center; color: var(--text2); }
  .i.ok { background: var(--accent); color: var(--on-accent, #fff); margin-left: 2px; }
  .sep { width: 1px; height: 16px; margin: 0 4px; background: var(--line2); }
  .trow { display: flex; align-items: center; gap: 12px; padding: 8px 16px; }
  .trow + .trow { border-top: 1px solid var(--line); }
  .ic {
    width: 28px; height: 28px; border-radius: 7px; background: var(--sunken); color: var(--text2);
    display: inline-flex; align-items: center; justify-content: center;
  }
  .lbl { flex: 1; font-size: 13px; }
  .k { font: 12px/20px var(--font-mono, Consolas, monospace); color: var(--text2); padding: 0 8px; border-radius: 6px; background: var(--sunken); }
</style>
