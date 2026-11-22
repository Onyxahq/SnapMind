<script lang="ts">
  // 截图遮罩（编辑态的底座）。每个显示器一个窗口，常驻在屏幕外，截图时移进来。
  // M1.1 冻结帧与遮罩；M1.2 选区；M1.3 吸附窗口和控件；M1.4 放大镜与取色；M1.5 操作提示；
  // M1.6 工具条、复制 / 保存与逐级退出；M2.1 标注；M2.2 文字、马赛克 / 模糊；
  // M2.3 按原始分辨率导出标注、未保存标注的退出确认；M6.1 翻译卡片（Ctrl+Q、Ctrl+Shift+O）；
  // M4.4 识别文字卡片（Shift+C、Ctrl+Shift+S）、检测框与右键划选；M6.2 翻译的原位覆盖；
  // M8.1 识别为 Markdown（Shift+Alt+R）；M8.2 表格识别、M8.3 公式识别（工具条「更多」）；M9.1 长截图（L）。
  import { onMount, tick } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import {
    captureCopy,
    capturePin,
    capturePinText,
    captureTranslate,
    captureTranslateOverlay,
    captureMarkdown,
    captureFormula,
    saveMarkdown,
    captureTable,
    saveXlsx,
    captureOcr,
    ocrCompose,
    ocrDownload,
    ocrForget,
    ocrStatus,
    onOcrDownload,
    longStart,
    longHit,
    longAuto,
    longWheel,
    longPreview,
    longCancel,
    longFinish,
    onLongProgress,
    type LongAxis,
    type LongFinish,
    type LongProgress,
    type OcrStatus,
    onTaskEvent,
    taskCancel,
    translateText,
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
  import { physicalToCss, unionRect, type Point, type Rect } from '$lib/geometry';
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
  import TranslateCard from './TranslateCard.svelte';
  import { bilingual, newCard, reduce, type TranslateCard as Card } from '$lib/task';
  import OcrCardView from './OcrCard.svelte';
  import OcrBoxes from './OcrBoxes.svelte';
  import TranslateOverlay from './TranslateOverlay.svelte';
  import MarkdownCard from './MarkdownCard.svelte';
  import FormulaCard from './FormulaCard.svelte';
  import LongBar from './LongBar.svelte';
  import { toDisplayMath } from '$lib/formula';
  import TableCard from './TableCard.svelte';
  import { parseHtml, toTsv, type TableCardState, type TableView } from '$lib/table';
  import { canvasMeasure, drawOverlay, drawable, fitText } from '$lib/overlay';
  import { newOcrCard, pickLines, type OcrCard } from '$lib/ocr';

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
  /** 翻译结果卡片（打开卡片不会结束截图） */
  let card = $state<Card | null>(null);
  /** Markdown 卡片（复用任务卡片的状态，src 是 Markdown 源码）；结果卡片同一时间只开一张 */
  let mdCard = $state<Card | null>(null);
  /** 公式卡片（src 是 LaTeX） */
  let fxCard = $state<Card | null>(null);
  /** 长截图（选区里透出真实页面，滚动时自动拼接） */
  let long = $state<{ axis: LongAxis; progress: LongProgress | null; preview: string | null; busy: boolean } | null>(null);
  let longBarRect = $state<Rect>();
  /** 表格卡片 */
  let tableCard = $state<TableCardState | null>(null);
  /** 翻译时的选区（原位覆盖只在选区没动时画） */
  let cardRegion = $state<Rect | null>(null);
  /** 识别文字卡片（和翻译卡片同一时间只开一张） */
  let ocr = $state<OcrCard | null>(null);
  /** 本地 OCR 模型的下载状态 */
  let ocrModel = $state<OcrStatus | null>(null);
  /** Ctrl+Shift+S：识别完成后直接复制 */
  let ocrAutoCopy = false;
  /** 右键划选检测框（本屏截图像素） */
  let rdrag = $state.raw<{ p0: Point; p: Point; downCss: Point; moved: boolean } | null>(null);
  /** 工具条位置：结果卡片要避开它 */
  let toolbarRect = $state<Rect>();

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
  /** 画标注时工具条保持显示（不随每一笔闪烁）；调整选区时隐藏。 */
  const showToolbar = $derived(sel !== null && !saveOpen && sel.width > 0 && sel.height > 0 && !drag);
  const selOnlyCss = $derived(sel ? cssRect(sel) : null);
  const sameRect = (a: Rect, b: Rect) => a.x === b.x && a.y === b.y && a.width === b.width && a.height === b.height;
  /** 检测框只在选区还是识别时那块时画（选区动了就对不上了） */
  const ocrBoxes = $derived(
    ocr?.result && sel && !saveOpen && !ann.tool && sameRect(sel, ocr.region) ? ocr.result : null,
  );
  /** 原位覆盖：覆盖模式、翻译完成、选区还是翻译时那块 */
  const overlayBlocks = $derived(
    config.value.translation.overlay && card?.overlay && sel && cardRegion && !saveOpen && sameRect(sel, cardRegion)
      ? drawable(card.overlay)
      : null,
  );
  const measureText = canvasMeasure();
  const overlayFits = $derived(overlayBlocks ? overlayBlocks.map((b) => fitText(b, measureText)) : []);
  /** 正在右键划的矩形（相对识别区域） */
  const rdragRect = $derived(
    rdrag?.moved && ocr
      ? {
          x: Math.min(rdrag.p0.x, rdrag.p.x) - ocr.region.x,
          y: Math.min(rdrag.p0.y, rdrag.p.y) - ocr.region.y,
          width: Math.abs(rdrag.p.x - rdrag.p0.x),
          height: Math.abs(rdrag.p.y - rdrag.p0.y),
        }
      : null,
  );
  /** 操作提示：内容随状态变化，避开已有选区（悬停高亮不算）。 */
  const hintList = $derived(
    hintsFor(
      long ? 'long' : ann.tool ? 'annotate' : ocrBoxes ? 'ocr' : sel ? 'selected' : 'idle',
      ann.tool ? TOOL_INFO[ann.tool].label : '',
    ),
  );

  /** 放大镜：框选前后都显示；移动选区时隐藏（标注、长截图、录屏时也要隐藏，后续任务接入）。 */
  const showMag = $derived(
    session !== null && frameReady && cursorPx !== null && drag?.kind !== 'move' && !saveOpen && !ann.tool && !card && !ocr && !mdCard && !fxCard && !tableCard && !long,
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
    if (long) {
      // 长截图：遮罩收到的滚轮转给下面的页面（像素增量 → 滚轮单位，100 像素约一格 120）
      e.preventDefault();
      const horizontal = Math.abs(e.deltaX) > Math.abs(e.deltaY);
      const delta = Math.round((horizontal ? e.deltaX : -e.deltaY) * 1.2);
      if (delta) longWheel(delta, horizontal);
      return;
    }
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
    closeCard();
    closeOcr();
    closeMd();
    closeFormula();
    tableCard = null;
    dropLong();
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
    closeCard();
    closeOcr();
    closeMd();
    closeFormula();
    tableCard = null;
    dropLong();
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
    // 原位覆盖的译文也随标注层一起交出去（画在标注下面）
    const blocks = overlayBlocks && sameRect(r, cardRegion!) ? overlayBlocks : null;
    const fits = overlayFits;
    if (!shapes.length && !blocks) return captureSetAnnotations(index, null);
    const t0 = performance.now();
    const under = blocks
      ? (g: CanvasRenderingContext2D) => {
          g.save();
          g.translate(r.x, r.y);
          drawOverlay(g, blocks, fits);
          g.restore();
        }
      : undefined;
    const layer = renderLayer(shapes, r, img, under);
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
      case 'translate': return translate();
      case 'ocr': return recognizeText();
      case 'markdown': return recognizeMarkdown();
      case 'table': return recognizeTable();
      case 'formula': return recognizeFormula();
      case 'long': return startLong();
    }
  }

  // ---------------------------------------------------------------- 翻译卡片

  let cardSeq = 0;

  /** Ctrl+Q / 工具条「翻译」：识别选区（还没有选区时用高亮元素或整屏）并翻译。 */
  function translate() {
    if (!session || drag) return;
    if (!sel) sel = region();
    closeOcr();
    closeMd();
    closeFormula();
    if (card) taskCancel(card.id);
    const id = `tr-${session.seq}-${++cardSeq}`;
    card = newCard(id);
    cardRegion = sel;
    const run = config.value.translation.overlay ? captureTranslateOverlay : captureTranslate;
    run(id, session.index, sel).catch((e) => {
      if (card?.id === id) card = { ...card, stage: 'error', error: String(e) };
    });
  }

  /** 卡片 / 覆盖切换（记进配置）。切到覆盖而这次还没有段落位置时，按覆盖模式重新翻译。 */
  function setTranslateMode(overlay: boolean) {
    updateConfig({ translation: { overlay } });
    const busy = card && (card.stage === 'recognize' || card.stage === 'oneshot' || card.stage === 'translate');
    if (overlay && card && !card.overlay && !busy) translate();
  }

  function retranslate() {
    if (!card || !card.src.trim()) return;
    // 改过原文后段落和位置对不上了：覆盖撤掉，结果回到卡片里
    card = { ...card, dst: '', stage: 'translate', backend: '', meta: '', error: '', overlay: null };
    translateText(card.id, card.src).catch((e) => {
      if (card) card = { ...card, stage: 'error', error: String(e) };
    });
  }

  function closeCard() {
    if (card) taskCancel(card.id);
    card = null;
  }

  /** 要交出去的文字：译文，或按设置用双语对照。 */
  function resultText(c: Card): string {
    return config.value.translation.bilingual ? bilingual(c.src, c.dst) : c.dst.trim();
  }

  /** 复制译文：交出结果，结束截图（不再询问未保存的标注）。覆盖模式下复制画上译文的截图。 */
  async function copyTranslation() {
    if (!card) return;
    if (overlayBlocks) return copy();
    await clipboardWriteText(resultText(card));
    close();
  }

  /** 把译文贴成文字卡片，结束截图。 */
  function pinTranslation() {
    if (!card || !session || !sel) return;
    if (overlayBlocks) return pin();
    const [i, r, text] = [session.index, sel, resultText(card)];
    resetView();
    capturePinText(i, r, text).catch((e) => log('error', `贴译文失败：${e}`));
  }

  // ---------------------------------------------------------------- 识别文字卡片

  /** Shift+C / 工具条「识别文字」/ Ctrl+Shift+S（autoCopy：识别完直接复制）。 */
  async function recognizeText(autoCopy = false) {
    if (!session || drag) return;
    if (!sel) sel = region();
    closeCard();
    closeOcr();
    closeMd();
    closeFormula();
    const id = `ocr-${session.seq}-${++cardSeq}`;
    const [index, r] = [session.index, sel];
    ocr = newOcrCard(id, r);
    ocrAutoCopy = autoCopy;
    const st = await ocrStatus();
    if (ocr?.id !== id) return;
    ocrModel = st;
    if (st.installed) runOcr(id, index, r);
    else ocr = { ...ocr, stage: 'model' };
  }

  async function runOcr(id: string, index: number, r: Rect) {
    if (ocr?.id === id) ocr = { ...ocr, stage: 'run', error: '' };
    try {
      const res = await captureOcr(id, index, r);
      if (ocr?.id !== id) return;
      ocr = { ...ocr, stage: 'done', result: res, text: res.text, pick: null };
      if (ocrAutoCopy && res.text.trim()) {
        await clipboardWriteText(res.text);
        toast('已复制识别出的文字');
      }
    } catch (e) {
      if (ocr?.id === id) ocr = { ...ocr, stage: 'error', error: String(e) };
    }
  }

  function downloadModel() {
    if (ocrModel) ocrModel = { ...ocrModel, downloading: true, received: 0, error: null };
    ocrDownload().catch((e) => log('warn', `下载 OCR 模型失败：${e}`));
  }

  /** 换排列方式或划选后，按段落规则重新拼接（改过的文字会被替换）。 */
  let composeSeq = 0;
  async function recompose(next: Partial<Pick<OcrCard, 'pick' | 'byLine'>>) {
    if (!ocr || ocr.stage !== 'done') return;
    const c = { ...ocr, ...next };
    ocr = c;
    const seq = ++composeSeq;
    try {
      const text = await ocrCompose(c.id, c.pick, c.byLine);
      // 期间又换了选择就丢掉这次结果（$state 会包一层代理，不能比较数组是否同一个）
      if (ocr?.id === c.id && seq === composeSeq) ocr = { ...ocr, text };
    } catch (e) {
      log('warn', `重新拼接识别文字失败：${e}`);
    }
  }

  function closeOcr() {
    if (ocr) ocrForget(ocr.id);
    ocr = null;
    rdrag = null;
  }

  /** 复制识别出的文字：交出结果，结束截图。 */
  async function copyOcr() {
    if (!ocr?.text.trim()) return;
    await clipboardWriteText(ocr.text.trim());
    close();
  }

  function pinOcr() {
    if (!ocr?.text.trim() || !session) return;
    const [i, r, text] = [session.index, ocr.region, ocr.text.trim()];
    resetView();
    capturePinText(i, r, text).catch((e) => log('error', `贴识别文字失败：${e}`));
  }

  // ---------------------------------------------------------------- Markdown 卡片

  /** Shift+Alt+R / 工具条「Markdown」：按识别链识别为 Markdown。 */
  function recognizeMarkdown() {
    if (!session || drag) return;
    if (!sel) sel = region();
    closeCard();
    closeOcr();
    closeMd();
    closeFormula();
    const id = `md-${session.seq}-${++cardSeq}`;
    mdCard = newCard(id, '', 'markdown');
    captureMarkdown(id, session.index, sel).catch((e) => {
      if (mdCard?.id === id) mdCard = { ...mdCard, stage: 'error', error: String(e) };
    });
  }

  function closeMd() {
    if (mdCard) taskCancel(mdCard.id);
    mdCard = null;
  }

  async function copyMarkdown() {
    if (!mdCard?.src.trim()) return;
    await clipboardWriteText(mdCard.src.trim());
    close();
  }

  async function saveMd() {
    if (!mdCard?.src.trim()) return;
    try {
      const path = await saveMarkdown(mdCard.src.trim() + '\n');
      if (path) close(); // 取消另存为：留在截图里
    } catch (e) {
      toast(`保存失败：${e}`, { kind: 'error' });
    }
  }

  // ---------------------------------------------------------------- 长截图

  /** L / 工具条「长截图」：选区固定，滚动页面时自动拼接（标注不带进长图，取消后还在）。 */
  async function startLong(axis: LongAxis = 'vertical') {
    if (!session || drag) return;
    if (!sel) sel = region();
    closeCard();
    closeOcr();
    closeMd();
    closeFormula();
    tableCard = null;
    ann.commitText();
    if (ann.tool) exitAnnotate();
    propsOpen = false;
    saveOpen = false;
    dropLong();
    long = { axis, progress: null, preview: null, busy: false };
    try {
      await longStart(session.index, sel, axis);
    } catch (e) {
      long = null;
      toast(`长截图失败：${e}`, { kind: 'error' });
    }
  }

  /** 只清掉界面状态（Rust 那边由调用方负责停止）。 */
  function dropLong() {
    if (long?.preview) URL.revokeObjectURL(long.preview);
    long = null;
    previewRev = 0;
  }

  function cancelLong() {
    longCancel();
    dropLong();
  }

  function setLongAxis(axis: LongAxis) {
    if (!long || long.axis === axis) return;
    startLong(axis);
  }

  async function finishLong(action: LongFinish) {
    if (!long || long.busy || !session) return;
    let path: string | undefined;
    if (action === 'save' && config.value.save.ctrlS !== 'direct') {
      const info = await saveInfo();
      const picked = await pickSaveFile(info.dir, info.name, config.value.save.format);
      if (!picked) return; // 取消另存为：留在长截图里
      path = picked;
    }
    long = { ...long, busy: true };
    try {
      await longFinish(action, path);
      resetView();
    } catch (e) {
      const msg = String(e);
      toast(msg, { kind: 'error' });
      // 识别前就发现没有模型时长截图还在继续；其余错误时截图已经结束
      if (msg.includes('OCR 模型')) long = long && { ...long, busy: false };
      else resetView();
    }
  }

  /** 拼接预览：内容版本变了就取一张新的，同一时间只取一张。 */
  let previewRev = 0;
  let previewBusy = false;
  function refreshPreview() {
    const p = long?.progress;
    if (!long || !p || previewBusy || p.rev === previewRev) return;
    previewBusy = true;
    const rev = p.rev;
    const thickness = (long.axis === 'vertical' ? 140 : 98) * mon.scale;
    longPreview(thickness)
      .then((url) => {
        if (!long) {
          if (url) URL.revokeObjectURL(url);
          return;
        }
        if (long.preview) URL.revokeObjectURL(long.preview);
        long = { ...long, preview: url };
        previewRev = rev;
      })
      .catch(() => {})
      .finally(() => {
        previewBusy = false;
        setTimeout(refreshPreview, 120);
      });
  }

  // 工具条的位置报给 Rust：指针在上面时遮罩接收鼠标，别处鼠标落到页面上
  $effect(() => {
    if (!long || !longBarRect) return;
    const r = longBarRect;
    const k = mon.scale;
    const pad = 6;
    longHit([
      {
        x: Math.max(0, Math.floor((r.x - pad) * k)),
        y: Math.max(0, Math.floor((r.y - pad - 34) * k)), // 上方的提示气泡也算
        width: Math.ceil((r.width + 2 * pad) * k),
        height: Math.ceil((r.height + 2 * pad + 34) * k),
      },
    ]);
  });

  // ---------------------------------------------------------------- 公式卡片

  /** 工具条「更多 → 公式识别」：按识别链里的视觉模型识别成 LaTeX。 */
  function recognizeFormula() {
    if (!session || drag) return;
    if (!sel) sel = region();
    closeCard();
    closeOcr();
    closeMd();
    closeFormula();
    tableCard = null;
    const id = `fx-${session.seq}-${++cardSeq}`;
    fxCard = newCard(id, '', 'formula');
    captureFormula(id, session.index, sel).catch((e) => {
      if (fxCard?.id === id) fxCard = { ...fxCard, stage: 'error', error: String(e) };
    });
  }

  function closeFormula() {
    if (fxCard) taskCancel(fxCard.id);
    fxCard = null;
  }

  async function copyFormula(dollars: boolean) {
    const tex = fxCard?.src.trim();
    if (!tex) return;
    await clipboardWriteText(dollars ? toDisplayMath(tex) : tex);
    close();
  }

  // ---------------------------------------------------------------- 表格卡片

  /** 工具条「更多 → 表格识别」：按识别链识别选区里的表格。 */
  async function recognizeTable() {
    if (!session || drag) return;
    if (!sel) sel = region();
    closeCard();
    closeOcr();
    closeMd();
    closeFormula();
    const id = `tb-${session.seq}-${++cardSeq}`;
    const [index, r] = [session.index, sel];
    tableCard = { id, stage: 'run', rows: [], meta: '', error: '' };
    try {
      const res = await captureTable(index, r);
      if (tableCard?.id !== id) return;
      const rows = res.rows ?? (res.html ? parseHtml(res.html) : null);
      if (!rows?.length) tableCard = { ...tableCard, stage: 'error', error: '没有识别出表格' };
      else tableCard = { ...tableCard, stage: 'done', rows, meta: res.meta };
    } catch (e) {
      if (tableCard?.id !== id) return;
      const msg = String(e);
      if (msg.includes('表格模型还没有下载')) {
        ocrModel = await ocrStatus();
        tableCard = { ...tableCard, stage: 'model' };
      } else tableCard = { ...tableCard, stage: 'error', error: msg };
    }
  }

  function downloadTableModel() {
    if (ocrModel) ocrModel = { ...ocrModel, downloading: true, received: 0, error: null };
    ocrDownload(true).catch((e) => log('warn', `下载表格模型失败：${e}`));
  }

  async function copyTable(view: TableView, source: string) {
    if (!tableCard) return;
    await clipboardWriteText(view === 'grid' ? toTsv(tableCard.rows) : source);
    close();
  }

  async function exportTable() {
    if (!tableCard) return;
    try {
      const path = await saveXlsx(tableCard.rows);
      if (path) close(); // 取消另存为：留在截图里
    } catch (e) {
      toast(`导出失败：${e}`, { kind: 'error' });
    }
  }

  /** 贴图：在原位置钉一张贴图（带标注），结束截图。 */
  function pin() {
    if (drag) return;
    finish((i, r) => capturePin(i, r).then(String));
  }

  // ---------------------------------------------------------------- 鼠标

  function onDown(e: PointerEvent) {
    // 识别卡片打开时，右键在检测框上划选一部分行
    if (session && e.button === 2 && ocrBoxes) {
      const p = toPx(e.clientX, e.clientY);
      rdrag = { p0: p, p, downCss: { x: e.clientX, y: e.clientY }, moved: false };
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      return;
    }
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
    if (rdrag) {
      const moved = rdrag.moved || Math.hypot(e.clientX - rdrag.downCss.x, e.clientY - rdrag.downCss.y) >= CLICK_SLOP;
      rdrag = { ...rdrag, p, moved };
      return;
    }
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
    if (rdrag) {
      const area = rdragRect;
      const moved = rdrag.moved;
      rdrag = null;
      if (!ocr?.result) return;
      if (moved && area) {
        const pick = pickLines(ocr.result, area);
        if (pick.length) recompose({ pick });
      } else if (ocr.pick) recompose({ pick: null }); // 右键单击：恢复全部
      return;
    }
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
    // Ctrl+Shift+O：选好区域就直接识别并翻译
    if (sel && session?.intent === 'translate' && !card) translate();
    // Ctrl+Shift+S：选好区域就识别文字并复制
    if (sel && session?.intent === 'recognize' && !ocr) recognizeText(true);
  }

  /** 右键：标注时退出标注；有选区时取消选区（已有标注时不取消，免得标注丢失）；没有选区时退出截图。 */
  function onContext(e: MouseEvent) {
    e.preventDefault();
    if (!session || drag || ann.gesture) return;
    if (ocr) return; // 识别卡片打开时右键用来划选检测框，不取消选区
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
    if (long) {
      onLongKey(e);
      return;
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      if (propsOpen) propsOpen = false;
      else if (saveOpen) saveOpen = false;
      else if (card) closeCard();
      else if (ocr) closeOcr();
      else if (mdCard) closeMd();
      else if (fxCard) closeFormula();
      else if (tableCard) tableCard = null;
      else if (ann.tool) exitAnnotate();
      else requestClose();
      return;
    }
    if (saveOpen) return; // 保存窗口打开时，其他快捷键交给它
    // 在卡片的文本框里打字、选择、Ctrl+C 时，按键交给文本框
    const target = e.target as HTMLElement | null;
    if (target && (target.tagName === 'TEXTAREA' || target.tagName === 'INPUT')) return;
    const ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'q') {
      e.preventDefault();
      translate();
      return;
    }
    if ((ctrl && !e.shiftKey && !e.altKey && e.key.toLowerCase() === 'c') || (e.key === 'Enter' && !ctrl)) {
      e.preventDefault();
      if (drag) return;
      // 识别卡片打开时复制的是文字
      if (ocr?.stage === 'done') copyOcr();
      else if (!ocr) copy();
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
    // Shift+Alt+R：识别为 Markdown（有的键盘布局下 Alt 组合的 e.key 不是字母，所以也认 e.code）
    if (e.shiftKey && e.altKey && !ctrl && (e.code === 'KeyR' || e.key.toLowerCase() === 'r')) {
      e.preventDefault();
      recognizeMarkdown();
      return;
    }
    if (e.shiftKey && !ctrl && !e.altKey && e.key.toLowerCase() === 'c') {
      e.preventDefault();
      recognizeText();
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
      case 'l':
        e.preventDefault();
        if (!ann.tool) startLong();
        break;
    }
  }

  /** 长截图时的按键：Enter 完成并复制、Ctrl+T 贴图、Ctrl+S 保存、Shift+C 识别文字、Esc 取消。 */
  function onLongKey(e: KeyboardEvent) {
    const ctrl = e.ctrlKey || e.metaKey;
    const k = e.key.toLowerCase();
    let act: LongFinish | 'cancel' | null = null;
    if (e.key === 'Escape') act = 'cancel';
    else if (e.key === 'Enter' || (ctrl && !e.shiftKey && k === 'c')) act = 'copy';
    else if (ctrl && !e.shiftKey && k === 't') act = 'pin';
    else if (ctrl && !e.shiftKey && k === 's') act = 'save';
    else if (e.shiftKey && !ctrl && !e.altKey && k === 'c') act = 'ocr';
    else if (k === 'h' && !ctrl && !e.altKey && !e.shiftKey) {
      updateConfig({ capture: { showHints: !config.value.capture.showHints } });
    }
    if (!act) return;
    e.preventDefault();
    if (act === 'cancel') {
      if (!long?.busy) cancelLong();
    } else finishLong(act);
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
    const offLong = onLongProgress((p) => {
      if (!long) return;
      long = { ...long, progress: p };
      refreshPreview();
    });
    const offTask = onTaskEvent((e) => {
      if (card) card = reduce(card, e);
      if (mdCard) mdCard = reduce(mdCard, e);
      if (fxCard) fxCard = reduce(fxCard, e);
    });
    // 模型下载完成：卡片还在等模型就接着识别
    const offOcr = onOcrDownload((s) => {
      ocrModel = s;
      if (s.installed && s.tableInstalled && !s.downloading && tableCard?.stage === 'model') recognizeTable();
      if (s.installed && !s.downloading && ocr?.stage === 'model' && session) runOcr(ocr.id, session.index, ocr.region);
    });
    if (!inTauri()) devCapture().then(({ start: s, url }) => start(s, url));
    return () => {
      offStart.then((f) => f());
      offClaim.then((f) => f());
      offTask.then((f) => f());
      offLong.then((f) => f());
      offOcr.then((f) => f());
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<div
  class="stage"
  class:active={session !== null}
  class:frame-ready={frameReady}
  class:long={long !== null}
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
  {#if overlayBlocks && cardRegion}
    <TranslateOverlay region={cardRegion} scale={mon.scale} blocks={overlayBlocks} fits={overlayFits} />
  {/if}
  {#if sel && !long}
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
      label={long ? '长截图 · 滚动页面自动拼接' : sizeLabel}
      handles={sel !== null && !ann.tool && !long && (!drag || drag.kind !== 'create')}
      animate={!sel && !drag}
    />
  {/if}
  {#if long && selOnlyCss}
    <LongBar
      sel={selOnlyCss}
      progress={long.progress}
      axis={long.axis}
      preview={long.preview}
      busy={long.busy}
      bind:rect={longBarRect}
      onaxis={setLongAxis}
      onauto={(on) => longAuto(on)}
      onfinish={finishLong}
      oncancel={cancelLong}
    />
  {/if}
  {#if showToolbar && selOnlyCss && !long}
    <Toolbar
      sel={selOnlyCss}
      tool={ann.tool}
      opts={ann.opts}
      canUndo={ann.canUndo}
      bind:propsOpen
      bind:rect={toolbarRect}
      onaction={onAction}
      ontool={setTool}
      onopts={(t, o) => ann.setOpts(t, o)}
      onundo={() => ann.undo()}
    />
  {/if}
  {#if ocrBoxes && ocr}
    <OcrBoxes region={ocr.region} scale={mon.scale} result={ocrBoxes} byLine={ocr.byLine} pick={ocr.pick} drag={rdragRect} />
  {/if}
  {#if ocr && selOnlyCss && !saveOpen}
    <OcrCardView
      sel={unionRect(selOnlyCss, showToolbar ? toolbarRect : null)}
      card={ocr}
      model={ocrModel}
      ontext={(v) => ocr && (ocr = { ...ocr, text: v })}
      onmode={(byLine) => recompose({ byLine })}
      ondownload={downloadModel}
      oncopy={copyOcr}
      onpin={pinOcr}
      onclose={closeOcr}
    />
  {/if}
  {#if tableCard && selOnlyCss && !saveOpen}
    <TableCard
      sel={unionRect(selOnlyCss, showToolbar ? toolbarRect : null)}
      card={tableCard}
      model={ocrModel}
      onrows={(rows) => tableCard && (tableCard = { ...tableCard, rows })}
      ondownload={downloadTableModel}
      oncopy={copyTable}
      onexport={exportTable}
      onclose={() => (tableCard = null)}
    />
  {/if}
  {#if fxCard && selOnlyCss && !saveOpen}
    <FormulaCard
      sel={unionRect(selOnlyCss, showToolbar ? toolbarRect : null)}
      card={fxCard}
      onsrc={(v) => fxCard && (fxCard = { ...fxCard, src: v })}
      oncopy={copyFormula}
      onclose={closeFormula}
    />
  {/if}
  {#if mdCard && selOnlyCss && !saveOpen}
    <MarkdownCard
      sel={unionRect(selOnlyCss, showToolbar ? toolbarRect : null)}
      card={mdCard}
      onsrc={(v) => mdCard && (mdCard = { ...mdCard, src: v })}
      oncopy={copyMarkdown}
      onsave={saveMd}
      onclose={closeMd}
    />
  {/if}
  {#if card && selOnlyCss && !saveOpen}
    <TranslateCard
      sel={unionRect(selOnlyCss, showToolbar ? toolbarRect : null)}
      {card}
      bilingual={config.value.translation.bilingual}
      overlay={config.value.translation.overlay}
      onmode={setTranslateMode}
      onsrc={(v) => card && (card = { ...card, src: v })}
      ondst={(v) => card && (card = { ...card, dst: v })}
      onretranslate={retranslate}
      oncopy={copyTranslation}
      onpin={pinTranslation}
      onclose={closeCard}
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
  /* 长截图：不显示冻结帧，选区里透出真实页面 */
  .frame-ready.long img { visibility: hidden; }
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
