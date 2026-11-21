<script lang="ts">
  // 截图工具条（动作部分）：贴图、保存 | 退出、复制。标注工具在 M2 加到左边。
  // 按钮 30px、内边距 4px（需求 3.6）。
  import type { Rect } from '$lib/geometry';
  import { placeToolbar, type ToolbarAction as Action } from '$lib/toolbar';
  import IconButton from '$lib/ui/IconButton.svelte';


  let { sel, onaction }: { sel: Rect; onaction: (a: Action) => void } = $props();

  let width = $state(160);
  let height = $state(38);
  const pos = $derived(placeToolbar(sel, { width, height }, { width: innerWidth, height: innerHeight }));
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
  <IconButton icon="pin" label="贴图  Ctrl+T" onclick={() => onaction('pin')} />
  <IconButton icon="download" label="保存…  Ctrl+S" onclick={() => onaction('save')} />
  <span class="sep" aria-hidden="true"></span>
  <IconButton icon="close" label="退出  Esc" onclick={() => onaction('exit')} />
  <button type="button" class="copy" title="复制  Ctrl+C / Enter" aria-label="复制" onclick={() => onaction('copy')}>
    <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
  </button>
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
</style>
