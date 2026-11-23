<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 标注工具的样式面板内容：子类型、颜色（马赛克没有）、粗细 / 字号 / 强度。
  // 外层的玻璃卡片和摆放位置由宿主决定（截图工具条是浮动弹层，贴图工具条是排在工具条下方）。
  import { COLORS, SIZES, TOOL_INFO, type Tool, type ToolOpts } from '$lib/annotate';
  import Seg from '$lib/ui/Seg.svelte';

  let {
    tool,
    opts,
    onopts,
  }: { tool: Tool; opts: ToolOpts; onopts: (t: Tool, o: Partial<ToolOpts>) => void } = $props();

  const info = $derived(TOOL_INFO[tool]);
</script>

<div class="props-body">
  {#if info.subs}
    <Seg
      label={t('子类型')}
      options={info.subs.map((s, i) => ({ value: String(i), label: t(s) }))}
      value={String(opts.sub)}
      onchange={(v) => onopts(tool, { sub: Number(v) })}
    />
  {/if}
  {#if info.colors}
    <div class="colors">
      {#each COLORS as c, i (c.value)}
        <button
          type="button"
          class="cdot"
          class:on={opts.color === i}
          style:background={c.value}
          aria-label={c.label}
          title={t(c.label)}
          onclick={() => onopts(tool, { color: i })}
        ></button>
      {/each}
    </div>
  {/if}
  <div class="sizes">
    <span class="lbl">{tool === 'text' ? t('字号') : tool === 'mosaic' ? t('强度') : t('粗细')}</span>
    {#each SIZES as s, i (s)}
      <button
        type="button"
        class="size"
        class:on={opts.size === i}
        aria-label="{s}px"
        title="{s}px"
        onclick={() => onopts(tool, { size: i })}
      ><span style:width="{s * 2 + 2}px" style:height="{s * 2 + 2}px"></span></button>
    {/each}
  </div>
</div>

<style>
  .props-body { display: flex; flex-direction: column; gap: 10px; }
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
