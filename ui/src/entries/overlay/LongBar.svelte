<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 长截图（需求 3.9、M9.1）：选区下方的工具条 + 选区旁边的实时拼接预览。
  // 选区里透出真实页面，滚轮直接滚页面；只有这条工具条接收鼠标（位置报给 Rust 切换穿透）。
  import type { Rect } from '$lib/geometry';
  import type { LongAxis, LongFinish, LongProgress } from '$lib/ipc';
  import { placeToolbar } from '$lib/toolbar';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Seg from '$lib/ui/Seg.svelte';

  let {
    sel,
    progress,
    axis,
    preview,
    busy,
    rect = $bindable(),
    onaxis,
    onauto,
    onfinish,
    oncancel,
  }: {
    /** 选区（CSS 像素） */
    sel: Rect;
    progress: LongProgress | null;
    axis: LongAxis;
    /** 拼接预览图地址（还没有时为 null） */
    preview: string | null;
    /** 正在交出结果（识别文字可能要几秒） */
    busy: boolean;
    /** 工具条位置（CSS 像素），给 Rust 做鼠标穿透切换 */
    rect?: Rect;
    onaxis: (a: LongAxis) => void;
    onauto: (on: boolean) => void;
    onfinish: (a: LongFinish) => void;
    oncancel: () => void;
  } = $props();

  let width = $state(520);
  let height = $state(38);
  const pos = $derived(placeToolbar(sel, { width, height }, { width: innerWidth, height: innerHeight }));
  $effect(() => {
    rect = { x: pos.x, y: pos.y, width, height };
  });

  const auto = $derived(progress?.auto ?? false);
  const size = $derived(progress ? `${progress.width} × ${progress.height}` : '…');
  const notice = $derived(
    progress?.state === 'lost'
      ? t('对不上了：滚慢一点，或往回滚一点再继续')
      : progress?.state === 'full'
        ? t('已到长度上限（30000 像素）')
        : progress?.state === 'end'
          ? t('已经滚到底了')
          : '',
  );

  // ---- 预览：纵向时放在选区右边（放不下放左边），横向时放在选区上方（放不下放下方）----
  const PW = 140; // 纵向预览宽 / 横向预览高（CSS 像素）
  const prevBox = $derived.by(() => {
    const m = 8;
    const gap = 12;
    if (axis === 'vertical') {
      const h = Math.max(160, Math.min(sel.height, innerHeight - 2 * m));
      const right = sel.x + sel.width + gap;
      const x = right + PW + 16 <= innerWidth - m ? right : Math.max(m, sel.x - gap - PW - 16);
      const y = Math.min(Math.max(m, sel.y), innerHeight - h - m - 20);
      return { x, y, w: PW + 16, h: h + 20 };
    }
    const w = Math.max(200, Math.min(sel.width, innerWidth - 2 * m));
    const above = sel.y - gap - (PW * 0.7 + 30);
    const y = above >= m ? above : Math.min(innerHeight - m - (PW * 0.7 + 30), sel.y + sel.height + gap + height + 8);
    const x = Math.min(Math.max(m, sel.x), innerWidth - w - m);
    return { x, y, w, h: PW * 0.7 + 30 };
  });
  /** 预览里长图的显示尺寸（CSS 像素）：纵向按宽度 PW 缩放，横向按高度缩放 */
  const shown = $derived.by(() => {
    if (!progress) return { w: 0, h: 0 };
    if (axis === 'vertical') return { w: PW, h: (progress.height * PW) / Math.max(1, progress.width) };
    const th = PW * 0.7;
    return { w: (progress.width * th) / Math.max(1, progress.height), h: th };
  });
</script>

<div
  class="prev"
  class:h={axis === 'horizontal'}
  style:left="{prevBox.x}px"
  style:top="{prevBox.y}px"
  style:width="{prevBox.w}px"
  style:height="{prevBox.h}px"
  aria-hidden="true"
>
  <span class="cap">{t('拼接预览')}</span>
  <div class="win">
    <div class="doc" style:width="{shown.w}px" style:height="{shown.h}px">
      {#if preview}<img src={preview} alt="" />{:else}<span class="ph"></span>{/if}
    </div>
  </div>
</div>

<div
  class="bar"
  role="toolbar"
  tabindex="-1"
  aria-label={t('长截图工具条')}
  style:left="{pos.x}px"
  style:top="{pos.y}px"
  bind:clientWidth={width}
  bind:clientHeight={height}
  onpointerdown={(e) => e.stopPropagation()}
>
  {#if notice}
    <span class="notice" class:warn={progress?.state === 'lost'} role="status">{notice}</span>
  {/if}
  <span class="size" title={t('宽 × 已拼接的高（像素）')}>{size}</span>
  <Seg
    label={t('方向')}
    options={[
      { value: 'vertical', label: t('纵向') },
      { value: 'horizontal', label: t('横向') },
    ]}
    value={axis}
    onchange={onaxis}
  />
  <span class="sep" aria-hidden="true"></span>
  <IconButton
    icon={auto ? 'pause' : 'play'}
    label={auto ? t('停止自动滚动') : t('自动滚动（也可以自己用滚轮滚）')}
    on={auto}
    disabled={busy}
    onclick={() => onauto(!auto)}
  />
  <IconButton icon="scan" label={t('识别文字 Shift+C')} disabled={busy} onclick={() => onfinish('ocr')} />
  <IconButton icon="pin" label={t('贴图 Ctrl+T')} disabled={busy} onclick={() => onfinish('pin')} />
  <IconButton icon="download" label={t('保存 Ctrl+S')} disabled={busy} onclick={() => onfinish('save')} />
  <IconButton icon="close" label={t('取消长截图 Esc')} disabled={busy} onclick={oncancel} />
  <button type="button" class="done" disabled={busy} title={t('完成并复制 Enter')} onclick={() => onfinish('copy')}>
    {busy ? t('处理中…') : t('完成并复制')}
  </button>
</div>

<style>
  .bar {
    position: absolute;
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 4px 4px 4px 12px;
    border-radius: var(--r-panel);
    background: var(--glass2);
    backdrop-filter: var(--blur-1);
    box-shadow: var(--elev-sm);
    color: var(--text);
    white-space: nowrap;
    animation: in var(--d-in) var(--ease-in) both;
  }
  @keyframes in { from { opacity: 0; transform: translateY(calc(4px * var(--m))) scale(calc(1 - .04 * var(--m))); } }
  .size {
    min-width: 92px;
    margin-right: 6px;
    font-size: 12.5px;
    font-variant-numeric: tabular-nums;
    color: var(--text2);
    transition: color var(--d-hover) var(--ease-hover);
  }
  .sep { width: 1px; height: 18px; margin: 0 4px; background: var(--line2); }
  .done {
    height: 30px;
    margin-left: 4px;
    padding: 0 12px;
    border: 0;
    border-radius: var(--r-ctl);
    background: var(--accent);
    color: var(--on-accent, #fff);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: filter var(--d-hover) var(--ease-hover), transform var(--d-press, 90ms) var(--ease-hover);
  }
  .done:hover { filter: brightness(1.08); }
  .done:active { transform: scale(.97); }
  .done:disabled { opacity: .6; cursor: default; }
  .notice {
    position: absolute;
    left: 50%;
    bottom: calc(100% + 8px);
    transform: translateX(-50%);
    padding: 4px 10px;
    border-radius: 999px;
    font-size: 12px;
    background: var(--glass2);
    backdrop-filter: var(--blur-1);
    box-shadow: var(--elev-sm);
    color: var(--text2);
    animation: in var(--d-in) var(--ease-in) both;
  }
  .notice.warn { color: var(--danger); }
  .prev {
    position: absolute;
    z-index: 25;
    box-sizing: border-box;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    border-radius: 10px;
    background: var(--glass2);
    backdrop-filter: var(--blur-2);
    box-shadow: var(--elev);
    pointer-events: none;
    animation: in var(--d-in) var(--ease-in) both;
  }
  .cap { font-size: 11px; color: var(--text3); line-height: 14px; }
  .win {
    position: relative;
    flex: 1;
    overflow: hidden;
    border-radius: 4px;
    background: var(--sunken);
    box-shadow: 0 0 0 1px var(--line2);
  }
  /* 纵向：最新的部分贴着底边；横向：贴着右边 */
  .doc { position: absolute; left: 0; bottom: 0; transition: height 160ms var(--ease-hover), width 160ms var(--ease-hover); }
  .h .doc { left: auto; right: 0; top: 0; bottom: auto; }
  .doc img { display: block; width: 100%; height: 100%; }
  .ph {
    display: block; width: 100%; height: 100%;
    background: repeating-linear-gradient(180deg, var(--hover) 0 6px, transparent 6px 14px);
  }
  .h .ph { background: repeating-linear-gradient(90deg, var(--hover) 0 6px, transparent 6px 14px); }
</style>
