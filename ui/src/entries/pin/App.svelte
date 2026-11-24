<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  import { IS_MAC } from '$lib/hotkey';
  // 贴图窗口（需求第 6 节）：置顶、拖动移动、滚轮缩放、Ctrl+滚轮调不透明度、Ctrl+0 恢复 100%、
  // Ctrl+Shift+X 鼠标穿透（全局快捷键，在 Rust 里处理）、双击或 Esc 关闭。
  // 窗口比图片大一圈透明边距，用来画阴影；缩放由 Rust 改窗口尺寸。
  // M3.2：悬停时显示共用的贴图工具条（复制、保存、更多、关闭）；Ctrl+C / Ctrl+S；文字卡片。
  // M3.3：标注模式（空格或工具条「标注」）——复用截图的标注控制器，标注画在图片上，
  // 期间拖动是绘制、双击不关闭；完成 / 空格 / Esc 退出，标注保留，复制和保存时一起导出。
  // M4.5：贴出后后台自动识别，铺一层透明文字：在文字上拖动是选字（Ctrl+C 复制选中的文字），
  // 在空白处拖动才是移动贴图；在文字上双击是选词，不关闭贴图。
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import {
    inTauri,
    onPinAction,
    onPinThrough,
    pinClose,
    pinCopy,
    pinHover,
    pinInfo,
    pinOpacity,
    pinReady,
    pinSave,
    pinSetImage,
    pinThrough,
    pinZoom,
    pinKeep,
    pinSetAnnotations,
    pinOcr,
    pinCopyText,
    sendAnnotState,
    type PinOcr,
    type PinActionMsg,
  } from '$lib/ipc';
  import { Annotator } from '$lib/annotator.svelte';
  import { renderLayer, toolForKey } from '$lib/annotate';
  import PinLayer from './PinLayer.svelte';
  import PinText from './PinText.svelte';
  import TextEditor from '../overlay/TextEditor.svelte';
  import { log } from '$lib/log';
  import { PIN_MARGIN, opacityStep, wrapText, zoomFactor } from '$lib/pin';
  import ToastHost from '$lib/ui/ToastHost.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  let src = $state('');
  let natural = $state({ width: 1, height: 1 });
  let opacity = $state(1);
  let through = $state(false);
  /** 刚缩放 / 调过不透明度时显示比例标签一会儿 */
  let flash = $state(false);
  let flashTimer = 0;
  let img = $state<HTMLImageElement>();
  let winWidth = $state(innerWidth);
  let pinId = 0;
  /** 文字卡片的原文（贴剪贴板里的文字时） */
  let cardText: string | null = null;
  /** 自动识别的结果（透明文字层） */
  let ocr = $state.raw<PinOcr | null>(null);
  let textLayer = $state<HTMLDivElement>();

  /** 1 个 CSS 像素对应多少图片像素（标注线宽、字号按它换算，缩放后画出的粗细和屏幕上看到的一致） */
  const unit = () => natural.width / Math.max(1, winWidth - 2 * PIN_MARGIN);
  const ann = new Annotator({
    unit,
    clamp: (p) => ({
      x: Math.min(Math.max(p.x, 0), natural.width),
      y: Math.min(Math.max(p.y, 0), natural.height),
    }),
  });
  /** 窗口内 CSS 坐标 → 图片像素 */
  const toImg = (e: { clientX: number; clientY: number }) => ({
    x: (e.clientX - PIN_MARGIN) * unit(),
    y: (e.clientY - PIN_MARGIN) * unit(),
  });

  // 把标注状态同步给工具条（工具条在标注模式下显示标注工具）
  $effect(() => {
    sendAnnotState({ id: pinId, active: ann.tool !== null, tool: ann.tool, opts: ann.opts, canUndo: ann.canUndo });
  });

  function enterAnnotate() {
    if (ann.tool) return;
    ann.setTool(ann.lastTool);
    pinKeep(true);
  }

  function exitAnnotate() {
    if (!ann.tool) return;
    ann.exit();
    pinKeep(false);
    syncAnnotations(); // Alt+F4 等直接关闭时，历史里也能带上标注
  }

  /** 关闭贴图：先把标注交给 Rust，历史记录里的图片才带标注。 */
  async function close() {
    await syncAnnotations();
    await pinClose();
  }

  /** 导出前把标注按图片原始分辨率渲染好交给 Rust（没有标注时清除）。 */
  async function syncAnnotations() {
    ann.commitText();
    if (!ann.hasShapes || !img) return pinSetAnnotations(null);
    await pinSetAnnotations(renderLayer(ann.doc.shapes, { x: 0, y: 0, ...natural }, img));
  }

  /** 当前缩放：图片显示宽度（物理像素）/ 图片原始宽度 */
  const zoom = $derived(((winWidth - 2 * PIN_MARGIN) * devicePixelRatio) / natural.width);
  const label = $derived(t('{round}% · 不透明度 {round2}%', { round: Math.round(zoom * 100), round2: Math.round(opacity * 100) }));

  function showLabel() {
    flash = true;
    clearTimeout(flashTimer);
    flashTimer = window.setTimeout(() => (flash = false), 1200);
  }

  // ---- 拖动：按下后移动超过 3px 才交给系统拖动，这样双击仍然能关闭 ----
  let press: { x: number; y: number } | null = null;

  /** 在识别出的文字上（这时拖动是选字，不移动贴图） */
  const onText = (e: Event) => !ann.tool && !!(e.target as Element | null)?.closest?.('.ln');

  /** 拖选到的文字：[行号, 选中的部分]。首尾两行可能只选了一半。 */
  function selectedPieces(): [number, string][] {
    const sel = getSelection();
    if (!sel || sel.isCollapsed || !sel.rangeCount || !textLayer) return [];
    const r = sel.getRangeAt(0);
    const out: [number, string][] = [];
    for (const el of textLayer.querySelectorAll<HTMLElement>('.ln')) {
      if (!r.intersectsNode(el)) continue;
      const text = el.textContent ?? '';
      // 端点可能落在行里的文字节点上（偏移是字符），也可能落在行元素本身（偏移是子节点）
      const at = (node: Node, offset: number) => (node === el ? (offset > 0 ? text.length : 0) : offset);
      const s = el.contains(r.startContainer) ? at(r.startContainer, r.startOffset) : 0;
      const e = el.contains(r.endContainer) ? at(r.endContainer, r.endOffset) : text.length;
      if (e > s) out.push([Number(el.dataset.i), text.slice(s, e)]);
    }
    return out;
  }

  function onDown(e: PointerEvent) {
    if (e.button !== 0) return;
    if (onText(e)) return; // 交给浏览器选字
    getSelection()?.removeAllRanges();
    if (ann.editing) {
      ann.commitText();
      return;
    }
    if (ann.tool) {
      // 标注模式：拖动是绘制；阻止默认的焦点切换，新建的文字框才不会立刻失焦
      e.preventDefault();
      if (ann.down(toImg(e), e.shiftKey)) (e.currentTarget as Element).setPointerCapture(e.pointerId);
      return;
    }
    press = { x: e.screenX, y: e.screenY };
  }

  function onMove(e: PointerEvent) {
    if (ann.gesture) {
      ann.move(toImg(e), e.shiftKey);
      return;
    }
    if (!press || !(e.buttons & 1)) return;
    if (Math.hypot(e.screenX - press.x, e.screenY - press.y) < 3) return;
    press = null;
    if (inTauri()) getCurrentWindow().startDragging();
  }

  function setOpacity(v: number) {
    opacity = v;
    pinOpacity(v);
  }

  async function copy() {
    const pieces = selectedPieces();
    if (pieces.length) {
      try {
        await pinCopyText(pieces);
        toast(t('已复制选中的文字'));
      } catch (e) {
        toast(t('复制失败：{e}', { e }), { kind: 'error' });
      }
      return;
    }
    try {
      await syncAnnotations();
      await pinCopy(pinId);
      toast(cardText && !ann.hasShapes ? t('已复制文字') : t('已复制图片'));
    } catch (e) {
      toast(t('复制失败：{e}', { e }), { kind: 'error' });
    }
  }

  async function save() {
    try {
      await syncAnnotations();
      const path = await pinSave(pinId);
      if (path) toast(t('已保存'));
    } catch (e) {
      toast(t('保存失败：{e}', { e }), { kind: 'error' });
    }
  }

  /** 工具条发来的动作。 */
  function onAction(a: PinActionMsg) {
    switch (a.action) {
      case 'annotate': return enterAnnotate();
      case 'done': return exitAnnotate();
      case 'tool': return a.tool && ann.setTool(a.tool);
      case 'opts': return a.tool && a.opts && ann.setOpts(a.tool, a.opts);
      case 'undo': return ann.undo();
      case 'copy': return copy();
      case 'save': return save();
      case 'close': return close();
      case 'through': return pinThrough();
      case 'zoom100':
        pinZoom({ absolute: 1, x: innerWidth / 2, y: innerHeight / 2 });
        return showLabel();
      case 'opacity':
        if (a.value !== undefined) setOpacity(a.value);
        return showLabel();
    }
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    // Mac 触控板双指捏合送来的是带 ctrlKey 的滚轮事件，要当缩放；Mac 上调不透明度用 ⌘ + 滚轮
    if (IS_MAC ? e.metaKey : e.ctrlKey) {
      setOpacity(opacityStep(opacity, e.deltaY));
    } else {
      pinZoom({ factor: zoomFactor(e.deltaY), x: e.clientX, y: e.clientY });
    }
    showLabel();
  }

  function onKey(e: KeyboardEvent) {
    const ctrl = e.ctrlKey || e.metaKey;
    if (ann.editing) return; // 打字时快捷键交给编辑框
    if (ann.tool) {
      // 标注模式：空格 / Esc 退出标注，1–7 切工具，Ctrl+Z 撤销；Ctrl+C / Ctrl+S 照常
      if (e.key === 'Escape' || e.key === ' ') {
        e.preventDefault();
        return exitAnnotate();
      }
      if (ctrl && !e.shiftKey && e.key.toLowerCase() === 'z') {
        e.preventDefault();
        return ann.undo();
      }
      const t = !ctrl && !e.altKey && !e.shiftKey ? toolForKey(e.key) : null;
      if (t) {
        e.preventDefault();
        return ann.setTool(t);
      }
    } else if (e.key === ' ') {
      e.preventDefault();
      return enterAnnotate();
    }
    if (e.key === 'Escape') {
      e.preventDefault();
      // 有选中的文字时先取消选择
      const sel = getSelection();
      if (sel && !sel.isCollapsed) sel.removeAllRanges();
      else close();
    } else if (ctrl && !e.shiftKey && e.key.toLowerCase() === 'c') {
      e.preventDefault();
      copy();
    } else if (ctrl && !e.shiftKey && e.key.toLowerCase() === 's') {
      e.preventDefault();
      save();
    } else if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'x') {
      // 全局快捷键被别的程序占用时按键会落到这里
      e.preventDefault();
      pinThrough();
    } else if ((e.ctrlKey || e.metaKey) && e.key === '0') {
      e.preventDefault();
      pinZoom({ absolute: 1, x: innerWidth / 2, y: innerHeight / 2 });
      showLabel();
    }
  }

  /** 把文字排成卡片图片（按当前主题配色，物理像素分辨率），交给 Rust 作为这张贴图的图片。 */
  async function renderCard(text: string) {
    const dpr = devicePixelRatio || 1;
    const css = getComputedStyle(document.documentElement);
    const bg = css.getPropertyValue('--solid').trim() || '#fff';
    const fg = css.getPropertyValue('--text').trim() || '#0F172A';
    const font = `15px Inter, 'Segoe UI', 'Microsoft YaHei UI', sans-serif`;
    const pad = 16, lh = 24, maxW = 460;
    const probe = new OffscreenCanvas(1, 1).getContext('2d')!;
    probe.font = font;
    const lines = wrapText(text.trim(), maxW, (s) => probe.measureText(s).width).slice(0, 200);
    const textW = Math.max(...lines.map((l) => probe.measureText(l).width), 40);
    const w = Math.ceil((Math.min(textW, maxW) + pad * 2) * dpr);
    const h = Math.ceil((lines.length * lh + pad * 2) * dpr);
    const c = new OffscreenCanvas(w, h);
    const g = c.getContext('2d', { willReadFrequently: true })!;
    g.scale(dpr, dpr);
    g.fillStyle = bg;
    g.fillRect(0, 0, w / dpr, h / dpr);
    g.fillStyle = fg;
    g.font = font;
    g.textBaseline = 'middle';
    lines.forEach((l, i) => g.fillText(l, pad, pad + i * lh + lh / 2));
    const data = g.getImageData(0, 0, w, h).data;
    await pinSetImage(w, h, new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
    natural = { width: w, height: h };
  }

  /** 浏览器开发页：把假识别结果的文字真的画出来，用来对照透明文字层是否和字对齐。 */
  async function devPinSrc(): Promise<string> {
    const c = document.createElement('canvas');
    c.width = natural.width;
    c.height = natural.height;
    const g = c.getContext('2d')!;
    g.fillStyle = '#fff';
    g.fillRect(0, 0, c.width, c.height);
    g.fillStyle = '#1f2937';
    g.textBaseline = 'top';
    for (const l of (await pinOcr())?.lines ?? []) {
      g.font = `${l.height * 1.25}px Georgia, 'SimSun', serif`;
      g.fillText(l.text, l.x, l.y - l.height * 0.12, l.width);
    }
    return c.toDataURL();
  }

  onMount(() => {
    const off = onPinThrough((on) => (through = on));
    const offAction = onPinAction(onAction);
    const onResize = () => (winWidth = innerWidth);
    addEventListener('resize', onResize);
    (async () => {
      const info = await pinInfo();
      if (!info) return log('error', '找不到贴图数据');
      natural = info;
      pinId = inTauri() ? Number(getCurrentWindow().label.replace('pin-', '')) : 1;
      if (info.text) {
        cardText = info.text;
        await renderCard(info.text);
      }
      // 带上时间戳：文字卡片换过图，避免拿到缓存的占位图
      src = inTauri() ? `${convertFileSrc(`pin/${pinId}`, 'snapframe')}?t=${Date.now()}` : await devPinSrc();
      await new Promise((r) => requestAnimationFrame(r));
      try {
        await img!.decode();
      } catch (e) {
        log('error', `贴图解码失败：${e}`);
      }
      await pinReady();
      // 后台自动识别（不阻塞显示）；文字卡片和没下载模型时没有结果
      if (!info.text) {
        pinOcr()
          .then((r) => (ocr = r && r.lines.length ? r : null))
          .catch((e) => log('warn', `贴图自动识别失败：${e}`));
      }
    })();
    return () => {
      off.then((f) => f());
      offAction.then((f) => f());
      removeEventListener('resize', onResize);
    };
  });
</script>

<svelte:window onkeydown={onKey} />

<div
  class="frame"
  class:through
  class:annot={ann.tool !== null}
  class:pixel={zoom >= 2}
  style:cursor={ann.tool ? ann.cursor() : null}
  style:inset="{PIN_MARGIN}px"
  style:opacity
  role="img"
  aria-label={t('贴图')}
  onpointerdown={onDown}
  onpointermove={onMove}
  onpointerenter={() => pinHover(true, opacity)}
  onpointerleave={() => pinHover(false, opacity)}
  onpointerup={() => {
    press = null;
    if (ann.gesture) ann.up();
  }}
  ondblclick={(e) => !ann.tool && !onText(e) && close()}
  onwheel={onWheel}
>
  {#if src}
    <img bind:this={img} {src} alt="" draggable="false" />
  {/if}
  <PinLayer shapes={ann.visible} draft={ann.draft} {natural} src={img} />
  {#if ocr}
    <PinText {ocr} {natural} css={(winWidth - 2 * PIN_MARGIN) / natural.width} disabled={ann.tool !== null} bind:layer={textLayer} />
  {/if}
  {#if ann.editing}
    <TextEditor
      shape={ann.editing.shape}
      scale={unit()}
      oninput={(t) => ann.setEditingText(t)}
      oncommit={() => ann.commitText()}
    />
  {/if}
  <span class="chip" class:show={flash}>{label}</span>
</div>

<ToastHost />

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
  .frame:hover, .frame.annot { box-shadow: 0 0 0 2px var(--accent), 0 3px 9px rgba(0, 0, 0, .28); }
  .frame.through { cursor: default; }
  @keyframes pin-in {
    from { opacity: 0; transform: scale(calc(1 - .06 * var(--m))); }
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
