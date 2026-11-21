<script lang="ts">
  // 截图遮罩（编辑态的底座）。每个显示器一个窗口，窗口启动后常驻隐藏。
  // M1.1：冻结帧 + 遮罩淡入 + Esc 退出；选区、放大镜、工具条在 M1.2 起逐步加入。
  import { onMount, tick } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { onCaptureStart, overlayEnd, overlayReady, type CaptureStart } from '$lib/ipc';
  import { log } from '$lib/log';

  let session = $state<CaptureStart | null>(null);
  let src = $state('');
  /** 冻结帧已解码并显示。遮罩本身在收到事件的第一帧就出现，不等截图传过来。 */
  let frameReady = $state(false);
  let img: HTMLImageElement;

  async function start(s: CaptureStart) {
    session = s;
    frameReady = false;
    src = `${convertFileSrc(String(s.index), 'snapframe')}?s=${s.seq}`;
    // 窗口是透明的：帧到之前遮罩下面透出的就是真实桌面，和冻结帧一模一样。
    // 等两帧再回报：事件比窗口移到屏幕上略早到，第二帧时遮罩一定已经在屏幕上了。
    requestAnimationFrame(() => requestAnimationFrame(() => overlayReady(s.seq, 'mask')));
    try {
      await tick(); // 等 src 真正写进 <img> 再解码
      await img.decode(); // 解码完成再显示，避免半张图
    } catch (e) {
      log('warn', `截图 #${s.seq} 解码失败：${e}`); // 被下一次截图打断，或帧已被释放
      return;
    }
    if (session?.seq !== s.seq) return;
    frameReady = true;
    requestAnimationFrame(() => overlayReady(s.seq, 'frame'));
  }

  /** 先清空画面并等它真的画出来，再让 Rust 把窗口停回屏幕外，下次出现时不会闪出上一张图。 */
  async function close() {
    if (!session) return;
    session = null;
    frameReady = false;
    src = '';
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    await overlayEnd();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  }

  onMount(() => {
    const off = onCaptureStart(start);
    return () => off.then((f) => f());
  });
</script>

<svelte:window onkeydown={onKey} oncontextmenu={(e) => e.preventDefault()} />

<div class="stage" class:active={session !== null} class:frame-ready={frameReady}>
  <img bind:this={img} {src} alt="" draggable="false" crossorigin="anonymous" />
  <div class="scrim"></div>
</div>

<style>
  :global(html), :global(body) {
    background: transparent;
    overflow: hidden;
    height: 100%;
    user-select: none;
  }
  .stage {
    position: fixed;
    inset: 0;
    cursor: crosshair;
    visibility: hidden;
  }
  .stage.active { visibility: visible; }
  img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
    /* 1 个截图像素对应 1 个物理像素，不做平滑 */
    image-rendering: pixelated;
    visibility: hidden;
  }
  .frame-ready img { visibility: visible; }
  .scrim {
    position: absolute;
    inset: 0;
    background: var(--scrim);
    opacity: 0;
  }
  .active .scrim {
    opacity: 1;
    transition: opacity var(--d-scrim) var(--ease-scrim);
  }
</style>
