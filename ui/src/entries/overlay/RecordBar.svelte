<script lang="ts" module>
  export type RecPhase = 'setup' | 'rec' | 'paused' | 'saving' | 'saved';
</script>

<script lang="ts">
  // 录屏工具条（需求 3.10、M9.2）：放在选区上方居中（上方放不下就放在选区里靠上，遮罩不会被录进去）。
  // 录制前：MP4 / GIF、帧率、是否录系统声音、开始录制；录制中：计时、暂停 / 继续、停止；
  // 停止后显示「正在保存」「已保存」，随后结束截图。
  import type { Rect } from '$lib/geometry';
  import type { RecordOptions } from '$lib/ipc';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Seg from '$lib/ui/Seg.svelte';

  let {
    sel,
    phase,
    options,
    elapsed,
    savedName = '',
    rect = $bindable(),
    onoptions,
    onstart,
    onpause,
    onstop,
    oncancel,
  }: {
    /** 选区（CSS 像素） */
    sel: Rect;
    phase: RecPhase;
    options: RecordOptions;
    /** 已录制的毫秒数（不含暂停） */
    elapsed: number;
    savedName?: string;
    /** 工具条位置（CSS 像素），给 Rust 做鼠标穿透切换 */
    rect?: Rect;
    onoptions: (o: Partial<RecordOptions>) => void;
    onstart: () => void;
    onpause: (paused: boolean) => void;
    onstop: () => void;
    oncancel: () => void;
  } = $props();

  let width = $state(420);
  let height = $state(38);
  const pos = $derived.by(() => {
    const m = 8;
    const x = Math.min(Math.max(m, sel.x + sel.width / 2 - width / 2), innerWidth - width - m);
    // 选区上方还有尺寸标签（约 24 px）：工具条放在标签上面；放不下就放到选区下方，再不行放在选区里靠上
    const above = sel.y - height - 34;
    const below = sel.y + sel.height + 10;
    const y = above >= m ? above : below + height <= innerHeight - m ? below : Math.min(sel.y + 34, innerHeight - height - m);
    return { x, y };
  });
  $effect(() => {
    rect = { x: pos.x, y: pos.y, width, height };
  });

  const time = $derived.by(() => {
    const s = Math.floor(elapsed / 1000);
    const mm = String(Math.floor(s / 60)).padStart(2, '0');
    const ss = String(s % 60).padStart(2, '0');
    return s >= 3600 ? `${Math.floor(s / 3600)}:${mm.slice(-2)}:${ss}` : `${mm}:${ss}`;
  });
  const gif = $derived(options.format === 'gif');
  const fpsOptions = $derived(
    (gif ? [10, 15, 30] : [15, 30, 60]).map((v) => ({ value: String(v), label: `${v}` })),
  );
</script>

<div
  class="bar"
  class:live={phase !== 'setup'}
  role="toolbar"
  tabindex="-1"
  aria-label="录屏工具条"
  style:left="{pos.x}px"
  style:top="{pos.y}px"
  bind:clientWidth={width}
  bind:clientHeight={height}
  onpointerdown={(e) => e.stopPropagation()}
>
  {#if phase === 'setup'}
    <b class="title">录屏</b>
    <Seg
      label="格式"
      options={[
        { value: 'mp4', label: 'MP4 视频' },
        { value: 'gif', label: 'GIF 动图' },
      ]}
      value={options.format}
      onchange={(v) => onoptions({ format: v, fps: v === 'gif' ? Math.min(options.fps, 30) : options.fps })}
    />
    <span class="fps" title="帧率（fps）">
      <Seg label="帧率" options={fpsOptions} value={String(options.fps)} onchange={(v) => onoptions({ fps: Number(v) })} />
      <span class="unit">fps</span>
    </span>
    <IconButton
      icon={options.audio && !gif ? 'volume' : 'mute'}
      label={gif ? 'GIF 不含声音' : options.audio ? '录制系统声音（点击关闭）' : '不录声音（点击开启）'}
      on={options.audio && !gif}
      disabled={gif}
      onclick={() => onoptions({ audio: !options.audio })}
    />
    <button type="button" class="red" title="开始录制  R" onclick={onstart}><span class="dot"></span>开始录制</button>
  {:else if phase === 'rec' || phase === 'paused'}
    <span class="recdot" class:paused={phase === 'paused'} aria-hidden="true"></span>
    <b class="time" aria-live="off">{time}</b>
    <span class="meta">{gif ? 'GIF' : 'MP4'} · {options.fps} fps{!gif && options.audio ? ' · 声音' : ''}</span>
    <IconButton
      icon={phase === 'paused' ? 'play' : 'pause'}
      label={phase === 'paused' ? '继续  空格' : '暂停  空格'}
      onclick={() => onpause(phase !== 'paused')}
    />
    <IconButton icon="close" label="放弃录屏  Esc" onclick={oncancel} />
    <button type="button" class="red" title="停止并保存  R" onclick={onstop}><span class="sq"></span>停止</button>
  {:else if phase === 'saving'}
    <span class="spin" aria-hidden="true"></span>
    <span class="msg">正在保存{gif ? ' GIF' : ''}…</span>
  {:else}
    <svg class="ok" viewBox="0 0 24 24" width="17" height="17" aria-hidden="true"><path d="M5 12.5l4.5 4.5L19 7.5" /></svg>
    <span class="msg" title={savedName}>已保存 · 文件已复制，可直接粘贴</span>
  {/if}
</div>

<style>
  .bar {
    position: absolute;
    z-index: 30;
    display: flex;
    align-items: center;
    gap: 6px;
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
  .title { font-size: 13px; margin-right: 2px; }
  .fps { display: inline-flex; align-items: center; gap: 4px; }
  .unit { font-size: 12px; color: var(--text3); }
  .red {
    display: inline-flex; align-items: center; gap: 7px;
    height: 30px; padding: 0 12px; border: 0; border-radius: var(--r-ctl);
    background: #ef4444; color: #fff; font: inherit; font-size: 13px; font-weight: 600; cursor: pointer;
    transition: filter var(--d-hover) var(--ease-hover), transform var(--d-press, 90ms) var(--ease-hover);
  }
  .red:hover { filter: brightness(1.08); }
  .red:active { transform: scale(.97); }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: #fff; }
  .sq { width: 9px; height: 9px; border-radius: 2px; background: #fff; }
  .recdot { width: 10px; height: 10px; border-radius: 50%; background: #ef4444; animation: rp 1s ease-in-out infinite; }
  .recdot.paused { animation: none; background: var(--text3); }
  @keyframes rp { 50% { opacity: .35; transform: scale(calc(1 - .15 * var(--m))); } }
  .time { min-width: 44px; font-size: 13px; font-variant-numeric: tabular-nums; }
  .meta { font-size: 12px; color: var(--text3); margin-right: 2px; }
  .msg { font-size: 13px; color: var(--text2); padding-right: 8px; }
  .ok { fill: none; stroke: #22c55e; stroke-width: 2.2; stroke-linecap: round; stroke-linejoin: round; }
  .spin {
    width: 14px; height: 14px; border-radius: 50%;
    border: 2px solid var(--line2); border-top-color: var(--accent);
    animation: sp .8s linear infinite;
  }
  @keyframes sp { to { transform: rotate(360deg); } }
</style>
