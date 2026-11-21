// 动效曲线与 Svelte 过渡。数值与 tokens.css 和设计规范 §4 保持一致。

/**
 * 生成 CSS cubic-bezier(x1, y1, x2, y2) 对应的缓动函数，供 Svelte 的 JS 过渡使用。
 * 先用牛顿法由 x 反解参数 t，不收敛时退回二分法。
 */
export function cubicBezier(x1: number, y1: number, x2: number, y2: number): (x: number) => number {
  const cx = 3 * x1, bx = 3 * (x2 - x1) - cx, ax = 1 - cx - bx;
  const cy = 3 * y1, by = 3 * (y2 - y1) - cy, ay = 1 - cy - by;
  const sx = (t: number) => ((ax * t + bx) * t + cx) * t;
  const sy = (t: number) => ((ay * t + by) * t + cy) * t;
  const dx = (t: number) => (3 * ax * t + 2 * bx) * t + cx;

  const solve = (x: number) => {
    let t = x;
    for (let i = 0; i < 8; i++) {
      const err = sx(t) - x;
      if (Math.abs(err) < 1e-6) return t;
      const d = dx(t);
      if (Math.abs(d) < 1e-6) break;
      t -= err / d;
    }
    let lo = 0, hi = 1;
    t = x;
    for (let i = 0; i < 40; i++) {
      const v = sx(t);
      if (Math.abs(v - x) < 1e-6) break;
      if (v < x) lo = t; else hi = t;
      t = (lo + hi) / 2;
    }
    return t;
  };

  return (x: number) => (x <= 0 ? 0 : x >= 1 ? 1 : sy(solve(x)));
}

export const ease = {
  in: cubicBezier(0.2, 0.9, 0.3, 1.15),
  out: cubicBezier(0.4, 0, 1, 1),
  hover: cubicBezier(0.2, 0, 0, 1),
  switch: cubicBezier(0.2, 0.9, 0.3, 1.2),
};

export const dur = { in: 180, out: 120 };

/** 出现：缩放 0.96→1、淡入、上移 4px，180ms。 */
export function popIn(_node: Element, { y = 4 }: { y?: number } = {}) {
  return {
    duration: dur.in,
    easing: ease.in,
    css: (t: number, u: number) =>
      `opacity:${Math.min(1, t)};transform:translateY(${u * y}px) scale(${0.96 + 0.04 * t})`,
  };
}

/** 消失：淡出并缩到 0.98，120ms，比出现更快。 */
export function popOut(_node: Element) {
  return {
    duration: dur.out,
    easing: ease.out,
    css: (t: number) => `opacity:${t};transform:scale(${0.98 + 0.02 * t})`,
  };
}
