// 结果卡片（识别文字、翻译、表格、公式、Markdown）的摆放、拖动和拉伸。
// 第一次量到高度时按选区摆好（placeCard），之后内容变高只往回收进屏幕、不换边；
// 按住标题栏空白处拖动，按住四边 / 四角拉伸。用户拉过尺寸后卡片定高（加 .sized），
// 里面的主体区域用 flex 填满。选区变了就回到自动摆放（尺寸保留）。
import type { Rect } from './geometry';
import { placeCard } from './task';

export type Edges = { n: boolean; s: boolean; w: boolean; e: boolean };
type Box = { x: number; y: number; w: number; h: number };
type View = { width: number; height: number };

const MARGIN = 8;
const EDGE = 6;
const CORNER = 14;

/** 指针落在卡片哪条边 / 哪个角上（相对卡片左上角的坐标），都不是就返回 null */
export function hitEdges(px: number, py: number, w: number, h: number): Edges | null {
  const nearW = px < CORNER, nearE = w - px < CORNER, nearN = py < CORNER, nearS = h - py < CORNER;
  // 角是圆的（贴边那几个像素点不中卡片），所以四角各留一块 CORNER 见方的区域都算拉角
  if ((nearW || nearE) && (nearN || nearS)) return { w: nearW, e: nearE && !nearW, n: nearN, s: nearS && !nearN };
  const e: Edges = { w: px < EDGE, e: w - px < EDGE, n: py < EDGE, s: h - py < EDGE };
  // 贴着一条边、又在另一条边的角区里，就算两条边一起拉
  if (e.w || e.e) { e.n ||= nearN; e.s ||= nearS; }
  if (e.n || e.s) { e.w ||= nearW; e.e ||= nearE; }
  return e.n || e.s || e.w || e.e ? e : null;
}

export function edgeCursor(e: Edges | null): string {
  if (!e) return '';
  if ((e.n && e.w) || (e.s && e.e)) return 'nwse-resize';
  if ((e.n && e.e) || (e.s && e.w)) return 'nesw-resize';
  return e.n || e.s ? 'ns-resize' : 'ew-resize';
}

/** 按住某些边拖了 (dx, dy) 后的新框：不小于最小尺寸、不出屏幕，对边保持不动 */
export function resizeBox(start: Box, e: Edges, dx: number, dy: number, min: { w: number; h: number }, view: View): Box {
  let { x, y } = start;
  let r = start.x + start.w, b = start.y + start.h;
  if (e.e) r = Math.min(Math.max(r + dx, x + min.w), view.width - MARGIN);
  if (e.s) b = Math.min(Math.max(b + dy, y + min.h), view.height - MARGIN);
  if (e.w) x = Math.max(Math.min(x + dx, r - min.w), MARGIN);
  if (e.n) y = Math.max(Math.min(y + dy, b - min.h), MARGIN);
  return { x, y, w: r - x, h: b - y };
}

/** 把卡片收进屏幕 */
export function clampBox(x: number, y: number, w: number, h: number, view: View): { x: number; y: number } {
  return {
    x: Math.min(Math.max(x, MARGIN), Math.max(MARGIN, view.width - w - MARGIN)),
    y: Math.min(Math.max(y, MARGIN), Math.max(MARGIN, view.height - h - MARGIN)),
  };
}

export type FloatParams = { sel: Rect; width: number; minWidth?: number; minHeight?: number };

/** 标题栏里这些东西照常点，不触发拖动 */
const INTERACTIVE = 'button, input, textarea, select, a, label, [role="radio"], [role="radiogroup"], [contenteditable="true"], [contenteditable="plaintext-only"]';

const GRIP =
  '<svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"><path d="M9 3 3 9M9 6.5 6.5 9"/></svg>';

export function floatCard(el: HTMLElement, params: FloatParams) {
  let p = params;
  let selKey = key(p.sel);
  let placed = false;
  let x = 0, y = 0;
  /** 用户拉过的尺寸；null 表示还是默认宽度、高度随内容 */
  let size: { w: number; h: number } | null = null;

  const view = (): View => ({ width: innerWidth, height: innerHeight });
  const w = () => size?.w ?? p.width;
  const h = () => el.offsetHeight;

  function apply() {
    el.style.left = `${x}px`;
    el.style.top = `${y}px`;
    el.style.width = `${w()}px`;
    el.style.height = size ? `${size.h}px` : '';
    el.classList.toggle('sized', !!size);
  }

  function place() {
    el.style.width = `${w()}px`;
    const pos = placeCard(p.sel, { width: w(), height: h() }, view());
    x = pos.x;
    y = pos.y;
    placed = true;
    apply();
  }

  function settle() {
    if (!placed) return place();
    ({ x, y } = clampBox(x, y, w(), h(), view()));
    apply();
  }

  const grip = document.createElement('span');
  grip.setAttribute('aria-hidden', 'true');
  grip.innerHTML = GRIP;
  Object.assign(grip.style, {
    position: 'absolute', right: '3px', bottom: '3px', width: '10px', height: '10px', display: 'flex',
    color: 'var(--text3)', opacity: '.55', pointerEvents: 'none',
  });
  el.append(grip);

  type Drag = { kind: 'move' | 'size'; id: number; sx: number; sy: number; box: Box; edges: Edges | null };
  let drag: Drag | null = null;

  function localEdges(ev: PointerEvent) {
    const r = el.getBoundingClientRect();
    return hitEdges(ev.clientX - r.left, ev.clientY - r.top, r.width, r.height);
  }

  function down(ev: PointerEvent) {
    if (ev.button !== 0 || drag) return;
    const edges = localEdges(ev);
    const target = ev.target as Element;
    const onHeader = !!target.closest('header') && !target.closest(INTERACTIVE);
    if (!edges && !onHeader) return;
    ev.preventDefault(); // 不选中标题文字
    const box = { x, y, w: el.offsetWidth, h: el.offsetHeight };
    drag = { kind: edges ? 'size' : 'move', id: ev.pointerId, sx: ev.clientX, sy: ev.clientY, box, edges };
    el.setPointerCapture(ev.pointerId);
    if (!edges) el.style.cursor = 'grabbing';
    document.documentElement.style.cursor = el.style.cursor;
  }

  function move(ev: PointerEvent) {
    if (!drag) {
      // 卡片在截图舞台里，不设的话会继承舞台的十字光标
      el.style.cursor = edgeCursor(localEdges(ev)) || 'default';
      return;
    }
    if (ev.pointerId !== drag.id) return;
    ev.stopPropagation();
    const dx = ev.clientX - drag.sx, dy = ev.clientY - drag.sy;
    if (drag.kind === 'move') {
      ({ x, y } = clampBox(drag.box.x + dx, drag.box.y + dy, drag.box.w, drag.box.h, view()));
    } else {
      const min = { w: Math.min(p.minWidth ?? 280, drag.box.w), h: Math.min(p.minHeight ?? 160, drag.box.h) };
      const b = resizeBox(drag.box, drag.edges!, dx, dy, min, view());
      x = b.x; y = b.y;
      size = { w: b.w, h: b.h };
    }
    placed = true;
    apply();
  }

  function up(ev: PointerEvent) {
    if (!drag || ev.pointerId !== drag.id) return;
    ev.stopPropagation();
    drag = null;
    el.style.cursor = 'default';
    document.documentElement.style.cursor = '';
    if (el.hasPointerCapture(ev.pointerId)) el.releasePointerCapture(ev.pointerId);
  }

  function leave() {
    if (!drag) el.style.cursor = 'default';
  }

  // 标题栏空白处显示可拖的手势
  el.style.cursor = 'default';
  const header = el.querySelector('header');
  if (header) header.style.cursor = 'grab';

  el.addEventListener('pointerdown', down);
  el.addEventListener('pointermove', move);
  el.addEventListener('pointerup', up);
  el.addEventListener('pointercancel', up);
  el.addEventListener('pointerleave', leave);
  const ro = new ResizeObserver(() => {
    if (!drag) settle();
  });
  ro.observe(el);
  addEventListener('resize', settle);
  place();

  return {
    update(next: FloatParams) {
      p = next;
      const k = key(next.sel);
      if (k !== selKey) {
        selKey = k;
        place();
      } else settle();
    },
    destroy() {
      ro.disconnect();
      removeEventListener('resize', settle);
      document.documentElement.style.cursor = '';
      grip.remove();
    },
  };
}

function key(r: Rect) {
  return `${r.x},${r.y},${r.width},${r.height}`;
}
