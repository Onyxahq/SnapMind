<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 贴图上的透明文字层（需求第 6 节「自动识别」、M4.5）：每一行识别出的文字按笔画范围摆放，
  // 横向拉伸到和图里的字一样宽，文字本身透明，拖选时只看到选区底色。复制时由页面收集选中的部分。
  import type { PinOcr } from '$lib/ipc';

  let {
    ocr,
    natural,
    css,
    disabled,
    layer = $bindable(),
  }: {
    ocr: PinOcr;
    natural: { width: number; height: number };
    /** 1 个图片像素对应多少 CSS 像素（随缩放变化） */
    css: number;
    /** 标注模式时不响应鼠标 */
    disabled: boolean;
    layer?: HTMLDivElement;
  } = $props();

  const FONT = `'Segoe UI', 'Microsoft YaHei UI', sans-serif`;
  /** 选区底色上下各多出字高的 20%，不至于贴着笔画 */
  const PAD = 0.2;

  // 每行在 100px 字号下的宽度只算一次；之后字号跟着缩放变，横向拉伸比例不变
  const ratios = $derived.by(() => {
    const g = new OffscreenCanvas(1, 1).getContext('2d')!;
    g.font = `100px ${FONT}`;
    return ocr.lines.map((l) => {
      const w = g.measureText(l.text).width;
      return w > 0 ? (l.width / l.height) * (100 / w) : 1;
    });
  });
  const pct = (v: number, of: number) => `${(v / of) * 100}%`;
</script>

<div class="text" class:disabled bind:this={layer} aria-label={t('识别出的文字')}>
  {#each ocr.lines as l, i (i)}
    {@const h = l.height * css}
    <div
      class="ln"
      data-i={i}
      style:left={pct(l.x, natural.width)}
      style:top={pct(l.y - l.height * PAD, natural.height)}
      style:height={pct(l.height * (1 + 2 * PAD), natural.height)}
      style:font-size="{h}px"
      style:line-height="{h * (1 + 2 * PAD)}px"
      style:font-family={FONT}
      style:transform="scaleX({ratios[i]})"
    >{l.text}</div>
  {/each}
</div>

<style>
  .text {
    position: absolute;
    inset: 0;
    z-index: 2;
  }
  .text.disabled { pointer-events: none; }
  .ln {
    position: absolute;
    white-space: pre;
    color: transparent;
    transform-origin: 0 50%;
    cursor: text;
    user-select: text;
    -webkit-user-select: text;
  }
  .ln::selection {
    color: transparent;
    background: color-mix(in srgb, var(--accent) 38%, transparent);
  }
</style>
