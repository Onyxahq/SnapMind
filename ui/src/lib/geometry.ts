// 坐标换算。约定（编程计划 §8）：Rust 和数据一律用物理像素、虚拟桌面坐标；
// 只有前端绘制时换算一次到 CSS 像素。每个遮罩窗口正好盖住一个显示器，
// 所以「窗口内 CSS 坐标 × 该显示器缩放 + 显示器原点」就是虚拟桌面物理坐标。

export interface MonitorGeom {
  /** 显示器左上角，虚拟桌面物理坐标（可为负） */
  x: number;
  y: number;
  /** 物理像素尺寸 */
  width: number;
  height: number;
  /** 缩放比例，如 1、1.25、1.5、2 */
  scale: number;
}

export interface Point {
  x: number;
  y: number;
}

export interface Rect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** 窗口内 CSS 坐标 → 虚拟桌面物理坐标（取整到像素）。 */
export function cssToPhysical(m: MonitorGeom, p: Point): Point {
  return { x: m.x + Math.round(p.x * m.scale), y: m.y + Math.round(p.y * m.scale) };
}

/** 虚拟桌面物理坐标 → 窗口内 CSS 坐标。 */
export function physicalToCss(m: MonitorGeom, p: Point): Point {
  return { x: (p.x - m.x) / m.scale, y: (p.y - m.y) / m.scale };
}

/** 窗口内 CSS 坐标 → 该显示器截图里的像素坐标（相对显示器左上角，夹在图内）。 */
export function cssToFramePixel(m: MonitorGeom, p: Point): Point {
  const clamp = (v: number, max: number) => Math.min(Math.max(v, 0), max - 1);
  return {
    x: clamp(Math.floor(p.x * m.scale), m.width),
    y: clamp(Math.floor(p.y * m.scale), m.height),
  };
}

/** 显示器的 CSS 尺寸（遮罩窗口的视口大小）。 */
export function cssSize(m: MonitorGeom): { width: number; height: number } {
  return { width: m.width / m.scale, height: m.height / m.scale };
}

/** 找到包含某个物理坐标的显示器序号，不在任何显示器上返回 -1。 */
export function monitorAt(monitors: MonitorGeom[], p: Point): number {
  return monitors.findIndex(
    (m) => p.x >= m.x && p.y >= m.y && p.x < m.x + m.width && p.y < m.y + m.height,
  );
}
