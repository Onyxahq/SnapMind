<script lang="ts">
  // 截图遮罩（编辑态的底座）。每个显示器一个窗口，常驻在屏幕外，截图时移进来。
  // M1.1 冻结帧与遮罩；M1.2 选区；M1.3 吸附窗口和控件；M1.4 放大镜与取色；M1.5 操作提示；
  // M1.6 工具条、复制 / 保存与逐级退出；M2.1 标注；M2.2 文字、马赛克 / 模糊；
  // M2.3 按原始分辨率导出标注、未保存标注的退出确认。
  import { onMount, tick } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import {
    captureCopy,
    capturePin,
    captureSetAnnotations,
    captureSave,
    pickSaveFile,
    saveInfo,
    claimSelection,
    clipboardWriteText,
    cursorNudge,
    inTauri,
    onCaptureStart,
    onSelectionClaimed,
    overlayEnd,
    overlayReady,
    snapChain,
    type CaptureStart,
    type Output,
    type Region,
    type Target,
  } from '$lib/ipc';
  import { containsPoint, keepLevel, toLocalChain, wheelLevel } from '$lib/snap';
  import { log } from '$lib/log';
  import { physicalToCss, type Point, type Rect } from '$lib/geometry';
  import { formatColor, nextFormat, wasdDelta, type RGB } from '$lib/color';
  import { config, updateConfig } from '$lib/config.svelte';
  import ToastHost from '$lib/ui/ToastHost.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import Magnifier from './Magnifier.svelte';
  import Hints from './Hints.svelte';
  import Toolbar from './Toolbar.svelte';
  import SaveDialog from './SaveDialog.svelte';
  import type { ToolbarAction } from '$lib/toolbar';
  import { hintsFor } from '$lib/hints';
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
  import AnnotationLayer from './AnnotationLayer.svelte';
  import TextEditor from './TextEditor.svelte';
  import ConfirmDiscard from './ConfirmDiscard.svelte';
  import { TOOL_INFO, renderLayer, toolForKey, type Tool } from '$lib/annotate';
  import { Annotator } from '$lib/annotator.svelte';
  import { devCapture, devChain } from './devFrame';

  /** 拖动时，按下点离开多少 CSS 像素才算「拖」，否则算单击。 */
  const CLICK_SLOP = 3;
  /** 控制点和边的命中容差（CSS 像素）。 */
  const HIT_TOL = 7;

  let session = $state<CaptureStart | null>(null);
  let src = $state('');
  /** 冻结帧已解码并显示。遮罩本身在收到事件的第一帧就出现，不等截图传过来。 */
  let frameReady = $state(false);
  let img = $state<HTMLImageElement>()!;

  /** 选区，单位为本显示器截图的物理像素。 */
  let sel = $state<Rect | null>(null);
  type Drag =
    | { kind: 'create'; anchor: Point; downCss: Point; moved: boolean }
    | { kind: 'move'; start: Rect; p0: Point }
    | { kind: 'resize'; handle: Handle; start: Rect; p0: Point };
  // raw：拖动过程中只整体替换，不需要深层响应
  let drag = $state.raw<Drag | null>(null);
  let hover = $state<Hit>(null);

  /** 光标的 CSS 坐标；鼠标离开这块屏时为 null。 */
  let cursorCss = $state<Point | null>(null);
  /** 放大镜当前像素的颜色（由放大镜组件回填）。 */
  let pickRgb = $state<RGB | null>(null);
  /** 保存窗口是否打开（逐级退出的一级）。 */
  let saveOpen = $state(false);
  /** 放弃标注的确认框（逐级退出的一级）。 */
  let confirmOpen = $state(false);

  // ---- 标注（状态与交互在共用的 Annotator 里；坐标是本屏截图像素）----
  /** 遮罩窗口常驻，所以各工具的样式整个运行期间都记得。 */
  const ann = new Annotator({ unit: () => mon.scale, clamp: (p) => clampToSel(p) });
  let propsOpen = $state(false);

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
  const cursor = $derived(
    ann.tool ? ann.cursor() : drag?.kind === 'move' ? 'move' : cursorFor(hover),
  );
  /** 光标所在的截图像素（向下取整：光标落在哪个像素里就是哪个） */
  const cursorPx = $derived(
    cursorCss
      ? {
          x: Math.min(Math.max(Math.floor(cursorCss.x * mon.scale), 0), W - 1),
          y: Math.min(Math.max(Math.floor(cursorCss.y * mon.scale), 0), H - 1),
        }
      : null,
  );
  /** 操作提示：内容随状态变化，避开已有选区（悬停高亮不算）。 */
  const hintList = $derived(
    hintsFor(ann.tool ? 'annotate' : sel ? 'selected' : 'idle', ann.tool ? TOOL_INFO[ann.tool].label : ''),
  );
  /** 画标注时工具条保持显示（不随每一笔闪烁）；调整选区时隐藏。 */
  const showToolbar = $derived(sel !== null && !saveOpen && sel.width > 0 && sel.height > 0 && !drag);
  const selOnlyCss = $derived(sel ? cssRect(sel) : null);

  /** 放大镜：框选前后都显示；移动选区时隐藏（标注、长截图、录屏时也要隐藏，后续任务接入）。 */
  const showMag = $derived(
    session !== null && frameReady && cursorPx !== null && drag?.kind !== 'move' && !saveOpen && !ann.tool,
  );

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
    saveOpen = false;
    confirmOpen = false;
    ann.reset();
    propsOpen = false;
    cursorCss = s.cursor ? physicalToCss(s.monitor, s.cursor) : null;
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

  /** 清空本窗口的画面和状态。 */
  function resetView() {
    session = null;
    sel = null;
    drag = null;
    saveOpen = false;
    confirmOpen = false;
    ann.reset();
    propsOpen = false;
    frameReady = false;
    src = '';
  }

  /** 退出截图；有未保存的标注时先确认（设置里可以关掉确认）。 */
  function requestClose() {
    ann.commitText();
    if (ann.hasShapes && config.value.capture.confirmDiscard) confirmOpen = true;
    else close();
  }

  function confirmDiscard(dontAsk: boolean) {
    confirmOpen = false;
    if (dontAsk) updateConfig({ capture: { confirmDiscard: false } });
    close();
  }

  /** 先清空画面并等它真的画出来，再让 Rust 把窗口停回屏幕外，下次出现时不会闪出上一张图。 */
  async function close() {
    if (!session) return;
    resetView();
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    await overlayEnd();
  }

  // ---------------------------------------------------------------- 交出结果

  /** 要交出去的区域：选区；还没有选区时用悬停高亮的元素，都没有就整屏。 */
  function region(): Region {
    return sel ?? snapList[snapLevel] ?? { x: 0, y: 0, width: W, height: H };
  }

  /** 把标注层按原始分辨率渲染好上传给 Rust（没有标注时清除），导出前调用。 */
  async function syncAnnotations(index: number, r: Region) {
    ann.commitText();
    const shapes = ann.doc.shapes;
    if (!shapes.length) return captureSetAnnotations(index, null);
    const t0 = performance.now();
    const layer = renderLayer(shapes, r, img);
    const t1 = performance.now();
    await captureSetAnnotations(index, layer);
    const t2 = performance.now();
    const size = layer ? `${layer.width}×${layer.height}` : '空';
    log('info', `标注层（选区 ${r.width}×${r.height}，内容 ${size}）：渲染 ${(t1 - t0).toFixed(0)} ms，上传 ${(t2 - t1).toFixed(0)} ms`);
  }

  /** 交出结果的动作都会结束截图：Rust 裁好图后就把遮罩移走，这里同步清空画面。 */
  async function finish(job: (index: number, r: Region) => Promise<string>) {
    if (!session) return;
    const index = session.index;
    const r = region();
    try {
      await syncAnnotations(index, r);
      const p = job(index, r);
      resetView();
      await p;
    } catch (e) {
      log('error', `交出截图失败：${e}`);
    }
  }

  const copy = () => finish((i, r) => captureCopy(i, r));

  function outputFor(r: Region): Output {
    const s = config.value.save;
    return { format: s.format, quality: s.quality, width: r.width, height: r.height };
  }

  /** Ctrl+S / 保存按钮：按设置打开保存窗口、系统另存为，或直接存到默认目录。 */
  async function save() {
    if (!session) return;
    switch (config.value.save.ctrlS) {
      case 'window':
        if (!sel) sel = region();
        // 保存窗口里的预计大小也要算上标注
        await syncAnnotations(session.index, sel);
        saveOpen = true;
        break;
      case 'direct':
        finish((i, r) => captureSave(i, r, outputFor(r), {}));
        break;
      case 'system': {
        const r = region();
        const info = await saveInfo();
        systemSave(outputFor(r), info.dir, info.name, r);
        break;
      }
    }
  }

  async function systemSave(output: Output, dir: string, name: string, r = region()) {
    const path = await pickSaveFile(dir, name, output.format);
    if (!path) return; // 取消：留在截图里
    finish((i) => captureSave(i, r, output, { path }));
  }

  function saveFromDialog(output: Output, target: Target) {
    finish((i, r) => captureSave(i, r, output, target));
  }

  function onAction(a: ToolbarAction) {
    switch (a) {
      case 'copy': return copy();
      case 'save': return save();
      case 'exit': return requestClose();
      case 'pin': return pin();
    }
  }

  /** 贴图：在原位置钉一张贴图（带标注），结束截图。 */
  function pin() {
    if (drag) return;
    finish((i, r) => capturePin(i, r).then(String));
  }

  // ---------------------------------------------------------------- 鼠标

  function onDown(e: PointerEvent) {
    if (!session || e.button !== 0) return;
    const p = toPx(e.clientX, e.clientY);
    if (ann.editing) {
      // 点别处：提交正在编辑的文字（编辑框失焦也会提交，提交是幂等的）
      ann.commitText();
      return;
    }
    if (ann.tool && sel) {
      // 阻止默认的焦点切换：否则刚建好的文字编辑框会立刻失焦、被当成空文字丢掉
      e.preventDefault();
      propsOpen = false;
      if (ann.down(p, e.shiftKey)) (e.currentTarget as Element).setPointerCapture(e.pointerId);
      return;
    }
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
    cursorCss = { x: e.clientX, y: e.clientY };
    const p = toPx(e.clientX, e.clientY);
    if (ann.gesture) {
      ann.move(p, e.shiftKey);
      return;
    }
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
    if (ann.gesture) {
      ann.up();
      return;
    }
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

  /** 右键：标注时退出标注；有选区时取消选区（已有标注时不取消，免得标注丢失）；没有选区时退出截图。 */
  function onContext(e: MouseEvent) {
    e.preventDefault();
    if (!session || drag || ann.gesture) return;
    if (ann.tool) {
      exitAnnotate();
      return;
    }
    if (sel && ann.hasShapes) return;
    if (sel) {
      sel = null;
      querySnap(toPx(e.clientX, e.clientY)); // 回到悬停吸附
    } else requestClose();
  }

  // ---------------------------------------------------------------- 标注

  const clampToSel = (p: Point): Point =>
    sel
      ? {
          x: Math.min(Math.max(p.x, sel.x), sel.x + sel.width),
          y: Math.min(Math.max(p.y, sel.y), sel.y + sel.height),
        }
      : p;

  /** 进入标注模式（还没有选区时先选中悬停的元素或整屏）。 */
  function setTool(t: Tool) {
    if (!session) return;
    if (!sel) sel = region();
    ann.setTool(t);
    hover = null;
  }

  function exitAnnotate() {
    ann.exit();
    propsOpen = false;
  }

  // ---------------------------------------------------------------- 键盘

  function onKey(e: KeyboardEvent) {
    if (!session) return;
    // Esc 逐级退出：弹层 → 保存窗口 → 标注 → 截图（长截图 / 录屏、结果卡片随后续任务插入）
    if (ann.editing || confirmOpen) return; // 打字时、确认框打开时，按键交给它们
    if (e.key === 'Escape') {
      e.preventDefault();
      if (propsOpen) propsOpen = false;
      else if (saveOpen) saveOpen = false;
      else if (ann.tool) exitAnnotate();
      else requestClose();
      return;
    }
    if (saveOpen) return; // 保存窗口打开时，其他快捷键交给它
    const ctrl = e.ctrlKey || e.metaKey;
    if ((ctrl && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'c') || (e.key === 'Enter' && !ctrl)) {
      e.preventDefault();
      if (!drag) copy();
      return;
    }
    if (ctrl && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 's') {
      e.preventDefault();
      if (!drag) save();
      return;
    }
    if (ctrl && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 't') {
      e.preventDefault();
      pin();
      return;
    }
    if (ctrl && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'z') {
      e.preventDefault();
      ann.undo();
      return;
    }
    if (e.key === ' ' && !ctrl && !e.altKey) {
      e.preventDefault();
      if (drag || ann.gesture) return;
      if (ann.tool) exitAnnotate();
      else setTool(ann.lastTool);
      return;
    }
    const t = !ctrl && !e.altKey && !e.shiftKey ? toolForKey(e.key) : null;
    if (t) {
      e.preventDefault();
      if (!drag) setTool(t);
      return;
    }
    const d = nudgeDelta(e.key, e.shiftKey);
    if (d && sel && !drag && !ann.tool) {
      e.preventDefault();
      sel = move(sel, d.x, d.y, W, H);
      return;
    }
    if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return; // 带修饰键的留给其他快捷键
    const w = wasdDelta(e.key);
    if (w) {
      e.preventDefault();
      if (inTauri()) cursorNudge(w.x, w.y); // 系统会发出鼠标移动，放大镜和拖动都跟着走
      else if (cursorCss) cursorCss = { x: cursorCss.x + w.x / mon.scale, y: cursorCss.y + w.y / mon.scale };
      return;
    }
    switch (e.key.toLowerCase()) {
      case 'p':
        e.preventDefault();
        updateConfig({ capture: { colorFormat: nextFormat(config.value.capture.colorFormat) } });
        break;
      case 'c':
        e.preventDefault();
        copyColor();
        break;
      case 'h':
        e.preventDefault();
        updateConfig({ capture: { showHints: !config.value.capture.showHints } });
        break;
    }
  }

  async function copyColor() {
    if (!showMag || !pickRgb) return;
    const text = formatColor(pickRgb, config.value.capture.colorFormat);
    try {
      await clipboardWriteText(text);
      toast(`已复制 ${text}`);
    } catch (err) {
      toast('复制颜色失败', { kind: 'error' });
      log('warn', `复制颜色失败：${err}`);
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
  onpointerleave={() => (cursorCss = null)}
  onwheel={onWheel}
  oncontextmenu={onContext}
>
  <img bind:this={img} {src} alt="" draggable="false" crossorigin="anonymous" />
  <div class="scrim" class:snapping={!sel && !drag} style:clip-path={scrimClip}></div>
  {#if sel}
    <AnnotationLayer shapes={ann.visible} draft={ann.draft} clip={sel} {mon} src={img} />
  {/if}
  {#if ann.editing}
    <TextEditor
      shape={ann.editing.shape}
      scale={mon.scale}
      oninput={(text) => ann.setEditingText(text)}
      oncommit={() => ann.commitText()}
    />
  {/if}
  {#if selCss}
    <Selection
      rect={selCss}
      label={sizeLabel}
      handles={sel !== null && !ann.tool && (!drag || drag.kind !== 'create')}
      animate={!sel && !drag}
    />
  {/if}
  {#if showToolbar && selOnlyCss}
    <Toolbar
      sel={selOnlyCss}
      tool={ann.tool}
      opts={ann.opts}
      canUndo={ann.canUndo}
      bind:propsOpen
      onaction={onAction}
      ontool={setTool}
      onopts={(t, o) => ann.setOpts(t, o)}
      onundo={() => ann.undo()}
    />
  {/if}
  {#if saveOpen && sel && session}
    <SaveDialog
      {img}
      index={session.index}
      {sel}
      onsave={saveFromDialog}
      onsystem={(o, d, n) => systemSave(o, d, n)}
      oncancel={() => (saveOpen = false)}
    />
  {/if}
  {#if confirmOpen}
    <ConfirmDiscard count={ann.doc.shapes.length} onconfirm={confirmDiscard} oncancel={() => (confirmOpen = false)} />
  {/if}
  {#if session && config.value.capture.showHints && !saveOpen && !confirmOpen}
    <Hints hints={hintList} avoid={selOnlyCss} cursor={cursorCss} />
  {/if}
  {#if showMag && cursorPx && cursorCss}
    <Magnifier
      {img}
      {mon}
      px={cursorPx}
      {cursorCss}
      format={config.value.capture.colorFormat}
      bind:rgb={pickRgb}
    />
  {/if}
</div>

<ToastHost />

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
