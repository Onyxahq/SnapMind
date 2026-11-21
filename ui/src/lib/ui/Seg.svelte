<script lang="ts" generics="T extends string">
  // 分段控件。选中块在选项之间滑动（开关曲线，220ms）；首次渲染不做动画。
  type Option = { value: T; label: string };
  let {
    options,
    value = $bindable(),
    label,
    onchange,
  }: { options: Option[]; value: T; label?: string; onchange?: (v: T) => void } = $props();

  let wrap: HTMLDivElement;
  let thumb = $state({ x: 0, w: 0 });
  let ready = $state(false);

  function measure() {
    const i = options.findIndex((o) => o.value === value);
    const el = wrap?.children[i + 1] as HTMLElement | undefined; // children[0] 是滑块
    if (el) thumb = { x: el.offsetLeft, w: el.offsetWidth };
  }

  $effect(() => {
    void value;
    void options;
    measure();
    if (!ready) requestAnimationFrame(() => (ready = true));
  });

  function pick(v: T) {
    if (v === value) return;
    value = v;
    onchange?.(v);
  }

  function onKey(e: KeyboardEvent) {
    const i = options.findIndex((o) => o.value === value);
    const step = e.key === 'ArrowRight' ? 1 : e.key === 'ArrowLeft' ? -1 : 0;
    if (!step) return;
    e.preventDefault();
    const next = options[(i + step + options.length) % options.length];
    pick(next.value);
    (wrap.children[options.indexOf(next) + 1] as HTMLElement).focus();
  }
</script>

<div class="seg-wrap" role="radiogroup" aria-label={label} bind:this={wrap} onkeydown={onKey} tabindex="-1">
  <span
    class="thumb"
    class:ready
    style:transform="translateX({thumb.x - 2}px)"
    style:width="{thumb.w}px"
    aria-hidden="true"
  ></span>
  {#each options as o (o.value)}
    <button
      type="button"
      role="radio"
      aria-checked={o.value === value}
      tabindex={o.value === value ? 0 : -1}
      class="seg"
      class:on={o.value === value}
      onclick={() => pick(o.value)}>{o.label}</button
    >
  {/each}
</div>

<style>
  .seg-wrap {
    position: relative;
    display: inline-flex;
    background: var(--sunken);
    border-radius: var(--r-ctl);
    padding: 2px;
    flex: none;
    outline: none;
  }
  .thumb {
    position: absolute;
    left: 2px;
    top: 2px;
    bottom: 2px;
    border-radius: 6px;
    background: var(--seg-on);
    box-shadow: var(--seg-sh);
  }
  .thumb.ready {
    transition:
      transform var(--d-switch) var(--ease-switch),
      width var(--d-switch) var(--ease-switch);
  }
  .seg {
    position: relative;
    border: 0;
    background: transparent;
    font-size: 12px;
    line-height: 18px;
    padding: 5px 12px;
    border-radius: 6px;
    color: var(--text2);
    cursor: pointer;
    white-space: nowrap;
    transition: color var(--d-hover) var(--ease-hover);
  }
  .seg:hover { color: var(--text); }
  .seg.on { color: var(--text); }
  .seg:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--accent-soft); }
</style>
