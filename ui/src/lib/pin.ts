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

/** 贴图工具条和贴图之间的透明连接区（CSS 像素），与 Rust 端 pintools::BRIDGE_CSS 一致。 */
export const TOOLS_BRIDGE = 8;

/**
 * 文字卡片换行：按段落（换行符）分段，段内超宽时换行——拉丁文尽量在空格处断开，
 * 中日韩等没有空格的文字逐字断开。measure 返回一段文字的宽度。
 */
export function wrapText(text: string, maxWidth: number, measure: (s: string) => number): string[] {
  const out: string[] = [];
  for (const para of text.replace(/\r\n?/g, '\n').split('\n')) {
    if (para === '') {
      out.push('');
      continue;
    }
    let line = '';
    for (const ch of Array.from(para)) {
      const next = line + ch;
      if (measure(next) <= maxWidth || line === '') {
        line = next;
        continue;
      }
      // 超宽：拉丁文回退到最后一个空格处断开
      const sp = line.lastIndexOf(' ');
      if (ch !== ' ' && sp > 0 && /[A-Za-z0-9]$/.test(line)) {
        out.push(line.slice(0, sp));
        line = line.slice(sp + 1) + ch;
      } else {
        out.push(line.trimEnd());
        line = ch === ' ' ? '' : ch;
      }
    }
    out.push(line);
  }
  return out;
}
