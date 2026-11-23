// 结果卡片（识别文字、翻译、表格、公式、Markdown）的摆放、拖动和拉伸。
// 第一次量到高度时按选区摆好（placeCard），之后内容变高只往回收进屏幕、不换边；
// 按住标题栏空白处拖动，按住四边 / 四角拉伸。用户拉过尺寸后卡片定高（加 .sized），
// 里面的主体区域用 flex 填满。选区变了就回到自动摆放（尺寸保留）。
// 两种放大：fit 把文字框（data-fit）放大到不用滚动、尽量不挡选区；full 把表格（data-grid）居中放大到看得全。
// 回到 normal 时恢复原来的位置和大小。
import type { Rect } from './geometry';
import { reduced } from './motion';
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

/** 适配时依次试这些宽度，取第一个不太高的 */
const FIT_WIDTHS = [480, 560, 640, 720];
const FULL_RATIO = 0.94;
const SCROLLBAR = 12;

const overlaps = (a: Box, b: Box) => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

/** 卡片变成 w×h 后放哪：先原地长，再贴着原来的右边往左长，再按选区重新摆；都会挡住选区就原地收进屏幕 */
export function fitPlace(cur: Box, w: number, h: number, sel: Rect, view: View): { x: number; y: number } {
  const s: Box = { x: sel.x, y: sel.y, w: sel.width, h: sel.height };
  const here = clampBox(cur.x, cur.y, w, h, view);
  const left = clampBox(cur.x + cur.w - w, cur.y, w, h, view);
  const auto = placeCard(sel, { width: w, height: h }, view);
  // 本来就压着选区（用户拖过去的，或选区太大）：不换地方
  if (overlaps(cur, s)) return here;
  return [here, left, auto].find((p) => !overlaps({ ...p, w, h }, s)) ?? here;
}

export type FloatMode = 'normal' | 'fit' | 'full';
export type FloatParams = {
  sel: Rect;
  width: number;
  minWidth?: number;
  minHeight?: number;
  mode?: FloatMode;
  /** 内容变了就换一个值：fit / full 时重新量尺寸 */
  rev?: string;
  /** 放大时用户自己拉了尺寸，退出放大 */
  onmanual?: () => void;
  /** 展开时双击标题栏 */
  onrestore?: () => void;
  /** 文字框里有没有放不下的内容 */
  onoverflow?: (v: boolean) => void;
};

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
  let size: { w: number; h: number } | null = null as { w: number; h: number } | null;

  let mode: FloatMode = 'normal';
  /** 放大前的样子；moved：放大后拖动过，还原时留在拖到的位置 */
  let saved: { x: number; y: number; size: { w: number; h: number } | null; moved: boolean } | null = null as { x: number; y: number; size: { w: number; h: number } | null; moved: boolean } | null;
  let anim: Animation | null = null;
  let raf = 0;
  let wasOver = false;

  const view = (): View => ({ width: innerWidth, height: innerHeight });
  const w = () => size?.w ?? p.width;
  const h = () => size?.h ?? el.offsetHeight;

  function apply() {
    el.style.left = `${x}px`;
    el.style.top = `${y}px`;
    el.style.width = `${w()}px`;
    el.style.height = size ? `${size.h}px` : '';
    el.classList.toggle('sized', !!size);
    el.classList.toggle('fitted', mode === 'fit');
    el.classList.toggle('full', mode === 'full');
  }

  function place() {
    el.style.width = `${w()}px`;
    const pos = placeCard(p.sel, { width: w(), height: h() }, view());
    x = pos.x;
    y = pos.y;
    placed = true;
    apply();
  }

  const boxNow = (): Box => ({ x: el.offsetLeft, y: el.offsetTop, w: el.offsetWidth, h: el.offsetHeight });

  /** 从 from 平滑变到现在的样子 */
  function animate(from: Box) {
    anim?.cancel();
    anim = null;
    if (reduced()) return;
    const to = boxNow();
    const f = (b: Box) => ({ left: `${b.x}px`, top: `${b.y}px`, width: `${b.w}px`, height: `${b.h}px` });
    const a = el.animate([f(from), f(to)], { duration: 240, easing: 'cubic-bezier(.2, .9, .25, 1)' });
    anim = a;
    a.onfinish = () => {
      if (anim === a) anim = null;
      check();
    };
  }

  /** 量出文字框全部放下要多大：临时让每个文字框高度等于内容，量卡片 */
  function measureFit(): { w: number; h: number } | null {
    const fields = [...el.querySelectorAll<HTMLTextAreaElement>('textarea[data-fit]')];
    if (!fields.length) return null;
    const v = view();
    const maxH = v.height - 2 * MARGIN;
    const tops = fields.map((t) => t.scrollTop);
    el.classList.remove('sized');
    el.classList.add('fitted');
    el.style.maxHeight = 'none';
    el.style.height = '';
    for (const t of fields) Object.assign(t.style, { flex: 'none', minHeight: '0', overflow: 'hidden' });
    let out = { w: p.width, h: 0 };
    let heights: number[] = [];
    for (const w of [p.width, ...FIT_WIDTHS.filter((x) => x > p.width)]) {
      if (w > v.width - 2 * MARGIN && out.h) break;
      el.style.width = `${w}px`;
      for (const t of fields) t.style.height = '0px';
      heights = fields.map((t) => Math.max(60, t.scrollHeight + 3));
      fields.forEach((t, i) => (t.style.height = `${heights[i]}px`));
      out = { w, h: el.offsetHeight + 1 };
      if (out.h <= Math.min(maxH, w * 1.1)) break;
    }
    el.style.maxHeight = '';
    fields.forEach((t, i) => {
      Object.assign(t.style, { flex: '', minHeight: '', overflow: '', height: '' });
      t.style.setProperty('--h', `${heights[i]}px`);
      t.scrollTop = tops[i];
    });
    return { w: out.w, h: Math.min(out.h, maxH) };
  }

  /** 量出整张表格要多大：放不下先缩字号，再放不下才滚动 */
  function measureFull(): { w: number; h: number } {
    const v = view();
    const max = { w: Math.floor(v.width * FULL_RATIO), h: Math.floor(v.height * FULL_RATIO) };
    const grid = el.querySelector('[data-grid]');
    if (!grid) return size ?? { w: Math.min(900, max.w), h: Math.floor(v.height * 0.8) };
    el.classList.remove('sized');
    el.classList.add('full');
    Object.assign(el.style, { maxHeight: 'none', width: 'max-content', height: '' });
    let w = 0, hh = 0;
    for (const fs of [13, 12, 11]) {
      el.style.setProperty('--fs', `${fs}px`);
      // 内容宽度带小数，进一位再加 1，免得差零点几像素冒出滚动条
      w = Math.ceil(el.getBoundingClientRect().width) + 1;
      hh = el.offsetHeight + 1;
      if (w <= max.w && hh <= max.h) break;
    }
    el.style.maxHeight = '';
    const tallW = hh > max.h ? w + SCROLLBAR : w;
    const wideH = w > max.w ? hh + SCROLLBAR : hh;
    return { w: Math.min(Math.max(tallW, p.width), max.w), h: Math.min(wideH, max.h) };
  }

  /** 按当前模式重新量、重新摆 */
  function refit(smooth: boolean, replace = false) {
    const from = boxNow();
    anim?.cancel();
    anim = null;
    const v = view();
    if (mode === 'full') {
      size = measureFull();
      x = Math.round((v.width - size.w) / 2);
      y = Math.round((v.height - size.h) / 2);
    } else if (mode === 'fit') {
      const cur: Box = { x, y, w: w(), h: h() };
      const m = measureFit();
      if (m) {
        size = m;
        ({ x, y } = replace ? placeCard(p.sel, { width: m.w, height: m.h }, v) : fitPlace(cur, m.w, m.h, p.sel, v));
      }
    }
    apply();
    if (smooth) animate(from);
  }

  function setMode(next: FloatMode) {
    const from = boxNow();
    if (mode === 'normal') saved = { x, y, size, moved: false };
    mode = next;
    if (next !== 'normal') return refit(true);
    anim?.cancel();
    anim = null;
    const s = saved;
    saved = null;
    if (s) {
      size = s.size;
      if (!s.moved) ({ x, y } = s);
    }
    apply();
    ({ x, y } = clampBox(x, y, w(), h(), view()));
    apply();
    animate(from);
  }

  function check() {
    if (!p.onoverflow) return;
    const over = [...el.querySelectorAll<HTMLTextAreaElement>('textarea[data-fit]')].some(
      (t) => t.scrollHeight > t.clientHeight + 1 || t.scrollWidth > t.clientWidth + 1,
    );
    if (over !== wasOver) p.onoverflow((wasOver = over));
  }

  /** 内容变了：等 DOM 更新完再量 */
  function later(replace = false) {
    cancelAnimationFrame(raf);
    raf = requestAnimationFrame(() => {
      if (mode !== 'normal' && !drag) refit(false, replace);
      check();
    });
  }

  function onResize() {
    if (mode === 'full') refit(false);
    else settle();
  }

  function dbl(ev: MouseEvent) {
    const t = ev.target as Element;
    if (mode === 'full' && t.closest('header') && !t.closest(INTERACTIVE)) p.onrestore?.();
  }

  function settle() {
    if (anim) return;
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
    if (ev.button !== 0 || drag || mode === 'full') return;
    anim?.finish();
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
      el.style.cursor = (mode !== 'full' && edgeCursor(localEdges(ev))) || 'default';
      return;
    }
    if (ev.pointerId !== drag.id) return;
    ev.stopPropagation();
    const dx = ev.clientX - drag.sx, dy = ev.clientY - drag.sy;
    if (drag.kind === 'move') {
      if (saved) saved.moved = true;
      ({ x, y } = clampBox(drag.box.x + dx, drag.box.y + dy, drag.box.w, drag.box.h, view()));
    } else {
      const min = { w: Math.min(p.minWidth ?? 280, drag.box.w), h: Math.min(p.minHeight ?? 160, drag.box.h) };
      const b = resizeBox(drag.box, drag.edges!, dx, dy, min, view());
      x = b.x; y = b.y;
      size = { w: b.w, h: b.h };
      if (mode !== 'normal') {
        mode = 'normal';
        saved = null;
        p.onmanual?.();
      }
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
    check();
  });
  ro.observe(el);
  addEventListener('resize', onResize);
  el.addEventListener('dblclick', dbl);
  place();
  check();

  return {
    update(next: FloatParams) {
      const revChanged = next.rev !== p.rev;
      p = next;
      const k = key(next.sel);
      const selChanged = k !== selKey;
      selKey = k;
      const to = next.mode ?? 'normal';
      // 拉伸退出放大时组件的状态晚一步，这里已经是 normal 就不用再动
      if (to !== mode) setMode(to);
      else if (mode !== 'normal') {
        if (selChanged || revChanged) later(selChanged);
      } else if (selChanged) place();
      else settle();
      if (mode === 'normal' && revChanged) later();
    },
    destroy() {
      ro.disconnect();
      cancelAnimationFrame(raf);
      anim?.cancel();
      removeEventListener('resize', onResize);
      document.documentElement.style.cursor = '';
      grip.remove();
    },
  };
}

function key(r: Rect) {
  return `${r.x},${r.y},${r.width},${r.height}`;
}
