<script lang="ts">
  // 截图遮罩（编辑态的底座）。每个显示器一个窗口，常驻在屏幕外，截图时移进来。
  // M1.1 冻结帧与遮罩；M1.2 选区；M1.3 吸附窗口和控件。放大镜、提示、工具条在 M1.4 起加入。
  import { onMount, tick } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import {
    claimSelection,
    inTauri,
    onCaptureStart,
    onSelectionClaimed,
    overlayEnd,
    overlayReady,
    snapChain,
    type CaptureStart,
  } from '$lib/ipc';
  import { containsPoint, keepLevel, toLocalChain, wheelLevel } from '$lib/snap';
  import { log } from '$lib/log';
  import type { Point, Rect } from '$lib/geometry';
  import {
    cursorFor,
    fromCorners,
    hitTest,
    move,
    nudgeDelta,
    resize,
    type Handle,
    type Hit,
  } from '$lib/selection';
  import Selection from './Selection.svelte';
  import { devCapture, devChain } from './devFrame';

  /** 拖动时，按下点离开多少 CSS 像素才算「拖」，否则算单击。 */
  const CLICK_SLOP = 3;
  /** 控制点和边的命中容差（CSS 像素）。 */
  const HIT_TOL = 7;

  let session = $state<CaptureStart | null>(null);
  let src = $state('');
  /** 冻结帧已解码并显示。遮罩本身在收到事件的第一帧就出现，不等截图传过来。 */
  let frameReady = $state(false);
  let img: HTMLImageElement;

  /** 选区，单位为本显示器截图的物理像素。 */
  let sel = $state<Rect | null>(null);
  type Drag =
    | { kind: 'create'; anchor: Point; downCss: Point; moved: boolean }
    | { kind: 'move'; start: Rect; p0: Point }
    | { kind: 'resize'; handle: Handle; start: Rect; p0: Point };
  // raw：拖动过程中只整体替换，不需要深层响应
  let drag = $state.raw<Drag | null>(null);
  let hover = $state<Hit>(null);

  /** 吸附链（本屏像素，从内到外）和当前高亮的层级。只在还没有选区时生效。 */
  let snapList = $state.raw<Rect[]>([]);
  let snapLevel = $state(0);
  const snapRect = $derived(sel || drag ? null : (snapList[snapLevel] ?? null));

  const mon = $derived(session?.monitor ?? { x: 0, y: 0, width: 1, height: 1, scale: 1 });
  const W = $derived(mon.width);
  const H = $derived(mon.height);

  /** CSS 像素 → 本屏物理像素（选区用整数像素）。 */
  const toPx = (cx: number, cy: number): Point => ({
    x: Math.min(Math.max(Math.round(cx * mon.scale), 0), W),
    y: Math.min(Math.max(Math.round(cy * mon.scale), 0), H),
  });
  const cssRect = (r: Rect): Rect => ({
    x: r.x / mon.scale,
    y: r.y / mon.scale,
    width: r.width / mon.scale,
    height: r.height / mon.scale,
  });

  /** 显示出来的框：已有选区，或者悬停吸附的高亮。 */
  const shown = $derived(sel ?? snapRect);
  const selCss = $derived(shown ? cssRect(shown) : null);
  const sizeLabel = $derived(
    shown ? `${mon.x + shown.x},${mon.y + shown.y}   ${shown.width} × ${shown.height} px` : '',
  );
  /** 遮罩上挖出选区：外框和内框用 evenodd 规则组成一个多边形。 */
  const scrimClip = $derived.by(() => {
    if (!selCss || selCss.width === 0 || selCss.height === 0) return 'none';
    const { x, y, width: w, height: h } = selCss;
    return `polygon(evenodd, 0 0, 100% 0, 100% 100%, 0 100%, 0 0, ${x}px ${y}px, ${x + w}px ${y}px, ${x + w}px ${y + h}px, ${x}px ${y + h}px, ${x}px ${y}px)`;
  });
  const cursor = $derived(drag?.kind === 'move' ? 'move' : cursorFor(hover));

  // ---------------------------------------------------------------- 吸附

  /** 同一时间只有一个查询在路上；返回时鼠标已经移走，就再查一次最新位置。 */
  let snapBusy = false;
  let snapPending: Point | null = null;

  async function querySnap(p: Point) {
    if (!session) return;
    if (snapBusy) {
      snapPending = p;
      return;
    }
    snapBusy = true;
    const seq = session.seq;
    const gx = mon.x + p.x;
    const gy = mon.y + p.y;
    try {
      const chain = inTauri() ? await snapChain(gx, gy) : devChain({ x: gx, y: gy });
      if (session?.seq === seq && !sel) {
        const local = toLocalChain(chain, mon);
        snapLevel = keepLevel(local, snapList[snapLevel] ?? null);
        snapList = local;
      }
    } finally {
      snapBusy = false;
      const next = snapPending;
      snapPending = null;
      if (next) querySnap(next);
    }
  }

  function onWheel(e: WheelEvent) {
    if (!session || sel || drag || !snapList.length) return;
    e.preventDefault();
    snapLevel = wheelLevel(snapLevel, e.deltaY, snapList.length);
  }

  async function start(s: CaptureStart, url?: string) {
    session = s;
    sel = null;
    drag = null;
    hover = null;
    snapList = [];
    snapLevel = 0;
    frameReady = false;
    src = url ?? `${convertFileSrc(String(s.index), 'snapframe')}?s=${s.seq}`;
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
    sel = null;
    drag = null;
    frameReady = false;
    src = '';
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    await overlayEnd();
  }

  // ---------------------------------------------------------------- 鼠标

  function onDown(e: PointerEvent) {
    if (!session || e.button !== 0) return;
    const p = toPx(e.clientX, e.clientY);
    const hit = sel ? hitTest(sel, p, HIT_TOL * mon.scale) : null;
    if (sel && hit === 'move') {
      drag = { kind: 'move', start: sel, p0: p };
    } else if (sel && hit !== null && hit !== 'move') {
      drag = { kind: 'resize', handle: hit, start: sel, p0: p };
    } else {
      drag = { kind: 'create', anchor: p, downCss: { x: e.clientX, y: e.clientY }, moved: false };
      claimSelection(session.index); // 其他显示器上的选区作废
    }
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
  }

  function onMoveEvt(e: PointerEvent) {
    if (!session) return;
    const p = toPx(e.clientX, e.clientY);
    if (!drag) {
      if (sel) hover = hitTest(sel, p, HIT_TOL * mon.scale);
      else querySnap(p);
      return;
    }
    switch (drag.kind) {
      case 'create':
        if (!drag.moved) {
          const d = Math.hypot(e.clientX - drag.downCss.x, e.clientY - drag.downCss.y);
          if (d < CLICK_SLOP) return;
          drag = { ...drag, moved: true };
        }
        sel = fromCorners(drag.anchor, p, W, H);
        break;
      case 'move':
        sel = move(drag.start, p.x - drag.p0.x, p.y - drag.p0.y, W, H);
        break;
      case 'resize':
        sel = resize(drag.start, drag.handle, p.x - drag.p0.x, p.y - drag.p0.y, W, H);
        hover = drag.handle;
        break;
    }
  }

  function onUp(e: PointerEvent) {
    if (!drag) return;
    if (drag.kind === 'create' && !drag.moved) {
      // 单击：选中吸附到的窗口或控件；桌面空白处选中整个显示器
      const p = toPx(e.clientX, e.clientY);
      const snapped = snapList[snapLevel];
      sel = snapped && containsPoint(snapped, p) ? snapped : { x: 0, y: 0, width: W, height: H };
    } else if (sel && (sel.width === 0 || sel.height === 0)) {
      sel = null;
    }
    drag = null;
    hover = sel ? hitTest(sel, toPx(e.clientX, e.clientY), HIT_TOL * mon.scale) : null;
  }

  /** 右键：有选区时取消选区，没有时退出截图。 */
  function onContext(e: MouseEvent) {
    e.preventDefault();
    if (!session || drag) return;
    if (sel) {
      sel = null;
      querySnap(toPx(e.clientX, e.clientY)); // 回到悬停吸附
    } else close();
  }

  // ---------------------------------------------------------------- 键盘

  function onKey(e: KeyboardEvent) {
    if (!session) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      close();
      return;
    }
    const d = nudgeDelta(e.key, e.shiftKey);
    if (d && sel && !drag) {
      e.preventDefault();
      sel = move(sel, d.x, d.y, W, H);
    }
  }

  onMount(() => {
    const offStart = onCaptureStart((s) => start(s));
    const offClaim = onSelectionClaimed((index) => {
      if (session && index !== session.index) sel = null;
    });
    if (!inTauri()) devCapture().then(({ start: s, url }) => start(s, url));
    return () => {
      offStart.then((f) => f());
      offClaim.then((f) => f());
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<div
  class="stage"
  class:active={session !== null}
  class:frame-ready={frameReady}
  style:cursor
  role="application"
  aria-label="截图区域"
  onpointerdown={onDown}
  onpointermove={onMoveEvt}
  onpointerup={onUp}
  onwheel={onWheel}
  oncontextmenu={onContext}
>
  <img bind:this={img} {src} alt="" draggable="false" crossorigin="anonymous" />
  <div class="scrim" class:snapping={!sel && !drag} style:clip-path={scrimClip}></div>
  {#if selCss}
    <Selection
      rect={selCss}
      label={sizeLabel}
      handles={sel !== null && (!drag || drag.kind !== 'create')}
      animate={!sel && !drag}
    />
  {/if}
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
    visibility: hidden;
    touch-action: none;
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
  /* 悬停吸附时，挖洞随高亮框一起平滑过去（120ms）；拖动选区时逐帧跟手，不加过渡 */
  .active .scrim.snapping {
    transition:
      opacity var(--d-scrim) var(--ease-scrim),
      clip-path var(--d-snap) var(--ease-hover);
  }
</style>
