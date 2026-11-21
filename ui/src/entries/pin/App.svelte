<script lang="ts">
  // 贴图窗口（需求第 6 节）：置顶、拖动移动、滚轮缩放、Ctrl+滚轮调不透明度、Ctrl+0 恢复 100%、
  // Ctrl+Shift+X 鼠标穿透（全局快捷键，在 Rust 里处理）、双击或 Esc 关闭。
  // 窗口比图片大一圈透明边距，用来画阴影；缩放由 Rust 改窗口尺寸。
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { inTauri, pinClose, pinInfo, pinReady, pinThrough, pinZoom, onPinThrough } from '$lib/ipc';
  import { log } from '$lib/log';
  import { PIN_MARGIN, opacityStep, zoomFactor } from '$lib/pin';

  let src = $state('');
  let natural = $state({ width: 1, height: 1 });
  let opacity = $state(1);
  let through = $state(false);
  /** 刚缩放 / 调过不透明度时显示比例标签一会儿 */
  let flash = $state(false);
  let flashTimer = 0;
  let img = $state<HTMLImageElement>();
  let winWidth = $state(innerWidth);

  /** 当前缩放：图片显示宽度（物理像素）/ 图片原始宽度 */
  const zoom = $derived(((winWidth - 2 * PIN_MARGIN) * devicePixelRatio) / natural.width);
  const label = $derived(`${Math.round(zoom * 100)}% · 不透明度 ${Math.round(opacity * 100)}%`);

  function showLabel() {
    flash = true;
    clearTimeout(flashTimer);
    flashTimer = window.setTimeout(() => (flash = false), 1200);
  }

  // ---- 拖动：按下后移动超过 3px 才交给系统拖动，这样双击仍然能关闭 ----
  let press: { x: number; y: number } | null = null;

  function onDown(e: PointerEvent) {
    if (e.button !== 0) return;
    press = { x: e.screenX, y: e.screenY };
  }

  function onMove(e: PointerEvent) {
    if (!press || !(e.buttons & 1)) return;
    if (Math.hypot(e.screenX - press.x, e.screenY - press.y) < 3) return;
    press = null;
    if (inTauri()) getCurrentWindow().startDragging();
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    if (e.ctrlKey) {
      opacity = opacityStep(opacity, e.deltaY);
    } else {
      pinZoom({ factor: zoomFactor(e.deltaY), x: e.clientX, y: e.clientY });
    }
    showLabel();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      pinClose();
    } else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'x') {
      // 全局快捷键被别的程序占用时按键会落到这里
      e.preventDefault();
      pinThrough();
    } else if ((e.ctrlKey || e.metaKey) && e.key === '0') {
      e.preventDefault();
      pinZoom({ absolute: 1, x: innerWidth / 2, y: innerHeight / 2 });
      showLabel();
    }
  }

  onMount(() => {
    const off = onPinThrough((on) => (through = on));
    const onResize = () => (winWidth = innerWidth);
    addEventListener('resize', onResize);
    (async () => {
      const info = await pinInfo();
      if (!info) return log('error', '找不到贴图数据');
      natural = info;
      const id = getCurrentWindow().label.replace('pin-', '');
      src = convertFileSrc(`pin/${id}`, 'snapframe');
      await new Promise((r) => requestAnimationFrame(r));
      try {
        await img!.decode();
      } catch (e) {
        log('error', `贴图解码失败：${e}`);
      }
      await pinReady();
    })();
    return () => {
      off.then((f) => f());
      removeEventListener('resize', onResize);
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<div
  class="frame"
  class:through
  class:pixel={zoom >= 2}
  style:inset="{PIN_MARGIN}px"
  style:opacity
  role="img"
  aria-label="贴图"
  onpointerdown={onDown}
  onpointermove={onMove}
  onpointerup={() => (press = null)}
  ondblclick={() => pinClose()}
  onwheel={onWheel}
>
  {#if src}
    <img bind:this={img} {src} alt="" draggable="false" />
  {/if}
  <span class="chip" class:show={flash}>{label}</span>
</div>

<style>
  :global(html), :global(body) {
    background: transparent;
    overflow: hidden;
    height: 100%;
    user-select: none;
  }
  .frame {
    position: fixed;
    border-radius: 2px;
    overflow: hidden;
    cursor: move;
    background: #fff;
    /* 阴影要落在 10px 的透明边距之内 */
    box-shadow: 0 0 0 1px rgba(15, 23, 42, .14), 0 3px 9px rgba(0, 0, 0, .28);
    transition: box-shadow var(--d-hover) var(--ease-hover), opacity var(--d-hover) var(--ease-hover);
    animation: pin-in 220ms var(--ease-in) both;
  }
  .frame:hover { box-shadow: 0 0 0 2px var(--accent), 0 3px 9px rgba(0, 0, 0, .28); }
  .frame.through { cursor: default; }
  @keyframes pin-in {
    from { opacity: 0; transform: scale(.94); }
  }
  img {
    display: block;
    width: 100%;
    height: 100%;
  }
  .pixel img { image-rendering: pixelated; }
  .chip {
    position: absolute;
    left: 8px;
    top: 8px;
    font-size: 11px;
    line-height: 16px;
    padding: 3px 8px;
    border-radius: var(--r-tag);
    background: var(--chip);
    color: var(--chip-text);
    pointer-events: none;
    opacity: 0;
    transition: opacity var(--d-hover) var(--ease-hover);
    white-space: nowrap;
  }
  .frame:hover .chip, .chip.show { opacity: 1; }
</style>
