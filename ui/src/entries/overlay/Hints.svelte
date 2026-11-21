<script lang="ts">
  // 操作提示面板：默认左下角，被选区挡住时换角；光标靠近时淡出。
  import type { Point, Rect } from '$lib/geometry';
  import { nearPanel, placeHints, type Hint } from '$lib/hints';

  let {
    hints,
    avoid,
    cursor,
  }: { hints: Hint[]; avoid: Rect | null; cursor: Point | null } = $props();

  let width = $state(240);
  let height = $state(200);
  const panel = $derived({ width, height });
  const pos = $derived(placeHints(panel, { width: innerWidth, height: innerHeight }, avoid));
  const near = $derived(nearPanel(cursor, pos, panel));
</script>

<div
  class="hints"
  class:near
  style:left="{pos.x}px"
  style:top="{pos.y}px"
  bind:clientWidth={width}
  bind:clientHeight={height}
>
  {#each hints as h (h.k)}
    <div class="row"><span class="key"><span class="hk">{h.k}</span></span><span>{h.label}</span></div>
  {/each}
  <div class="foot">H 隐藏提示（会记住，下次也不显示）</div>
</div>

<style>
  .hints {
    position: absolute;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--chip);
    color: var(--chip-text);
    pointer-events: none;
    min-width: 200px;
    z-index: 10;
    animation: hints-in var(--d-in) var(--ease-in) both;
    transition:
      opacity var(--d-hover) var(--ease-hover),
      left var(--d-in) var(--ease-hover),
      top var(--d-in) var(--ease-hover);
  }
  .hints.near { opacity: .12; }
  @keyframes hints-in {
    from { opacity: 0; transform: translateY(4px) scale(.98); }
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    line-height: 20px;
    white-space: nowrap;
  }
  .key {
    display: flex;
    min-width: 96px;
    justify-content: flex-end;
  }
  .hk {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 22px;
    height: 20px;
    padding: 0 6px;
    border-radius: 5px;
    background: rgba(255, 255, 255, .14);
    border: 1px solid rgba(255, 255, 255, .18);
    font-size: 11px;
    font-weight: 600;
  }
  .foot {
    font-size: 11px;
    color: rgba(255, 255, 255, .55);
    padding-top: 3px;
    border-top: 1px solid rgba(255, 255, 255, .12);
    white-space: nowrap;
  }
</style>
