// 贴图窗口的纯逻辑。

/** 图片四周的透明边距（CSS 像素），用来画阴影。与 Rust 端 pin::MARGIN_CSS 一致。 */
export const PIN_MARGIN = 10;

/** 滚轮缩放倍数：向上放大 10%，向下缩小。 */
export function zoomFactor(deltaY: number): number {
  return deltaY < 0 ? 1.1 : 1 / 1.1;
}

/** Ctrl+滚轮调不透明度：每格 10%，范围 10%–100%。 */
export function opacityStep(current: number, deltaY: number): number {
  const next = Math.round((current + (deltaY < 0 ? 0.1 : -0.1)) * 10) / 10;
  return Math.min(1, Math.max(0.1, next));
}
