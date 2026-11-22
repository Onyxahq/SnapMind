<script lang="ts">
  // 本地 OCR 模型的状态和下载（识别与翻译页、新手引导共用）。进度由 Rust 广播，
  // 截图里提示下载时这里也同步。
  import { onMount } from 'svelte';
  import { ocrDownload, ocrStatus, onOcrDownload, type OcrStatus } from '$lib/ipc';
  import Row from '$lib/ui/Row.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Icon from '$lib/ui/Icon.svelte';

  let st = $state<OcrStatus | null>(null);
  onMount(() => {
    ocrStatus().then((s) => (st = s));
    const off = onOcrDownload((s) => (st = s));
    return () => {
      off.then((f) => f());
    };
  });

  const mb = (n: number) => (n / 1048576).toFixed(1);
  const pct = $derived(st && st.total ? Math.min(100, (st.received / st.total) * 100) : 0);

  function download() {
    if (st) st = { ...st, downloading: true, received: 0, error: null };
    // 失败原因随状态广播回来，显示在说明里
    ocrDownload().catch(() => {});
  }
</script>

<Row
  title="本地文字识别模型"
  desc={st?.error
    ? `下载失败：${st.error}`
    : '离线识别截图里的文字，原位翻译要靠它给出每行的位置。PP-OCRv5，下载一次即可'}
>
  {#if !st}
    <span></span>
  {:else if st.installed}
    <span class="ok"><Icon name="check" />已安装</span>
  {:else if st.downloading}
    <div class="dl" role="progressbar" aria-label="下载进度" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)}>
      <div class="bar"><i style:width="{pct}%"></i></div>
      <span>{mb(st.received)} / {mb(st.total)} MB</span>
    </div>
  {:else}
    <Button variant="primary" icon="download" onclick={download}>{st.error ? '重试' : `下载（${mb(st.total)} MB）`}</Button>
  {/if}
</Row>

<style>
  .ok {
    display: inline-flex; align-items: center; gap: 6px;
    font-size: 13px; color: var(--accent); font-weight: 600;
    animation: in var(--d-in) var(--ease-in) both;
  }
  .ok :global(svg) { width: 16px; height: 16px; }
  @keyframes in { from { opacity: 0; transform: scale(calc(1 - .2 * var(--m))); } }
  .dl { display: flex; flex-direction: column; align-items: flex-end; gap: 6px; width: 180px; }
  .dl span { font-size: 12px; color: var(--text2); font-variant-numeric: tabular-nums; }
  .bar { width: 100%; height: 6px; border-radius: 3px; background: var(--sunken); overflow: hidden; }
  .bar i { display: block; height: 100%; border-radius: 3px; background: var(--accent); transition: width 160ms linear; }
</style>
