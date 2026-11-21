<script lang="ts">
  // 保存窗口（需求 3.7）：左边预览，右边位置、文件名、格式、尺寸、质量与预计大小。
  // Enter 保存并结束截图；Esc 只关闭本窗口（逐级退出）。
  import { onMount, untrack } from 'svelte';
  import type { Rect } from '$lib/geometry';
  import {
    captureEstimate,
    openFolder,
    pickFolder,
    saveInfo,
    type ImageFormat,
    type Output,
    type SaveInfo,
    type Target,
  } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import { formatBytes, joinPath, lockedSize } from '$lib/toolbar';
  import Button from '$lib/ui/Button.svelte';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Field from '$lib/ui/Field.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import { popIn, popOut } from '$lib/motion';

  let {
    img,
    index,
    sel,
    onsave,
    onsystem,
    oncancel,
  }: {
    img: HTMLImageElement;
    index: number;
    /** 选区，本屏物理像素 */
    sel: Rect;
    onsave: (output: Output, target: Target) => void;
    onsystem: (output: Output, dir: string, name: string) => void;
    oncancel: () => void;
  } = $props();

  const FORMATS: { value: ImageFormat; label: string }[] = [
    { value: 'png', label: '.png' },
    { value: 'jpg', label: '.jpg' },
    { value: 'webp', label: '.webp' },
  ];
  const FORMAT_NOTE: Record<ImageFormat, string> = {
    png: '无损，文字和界面最清晰，体积较大',
    jpg: '有损压缩，体积小，适合照片；不支持透明',
    webp: '同等清晰度下比 JPG 更小，新版软件都能打开',
  };

  let info = $state<SaveInfo | null>(null);
  let dir = $state('');
  let name = $state('');
  let format = $state<ImageFormat>(untrack(() => config.value.save.format));
  let quality = $state(untrack(() => config.value.save.quality));
  let width = $state(untrack(() => sel.width));
  let height = $state(untrack(() => sel.height));
  let lock = $state(true);
  let bytes = $state<number | null>(null);
  let nameInput = $state<HTMLInputElement>();
  let preview: HTMLCanvasElement;

  const output = $derived<Output>({ format, quality, width, height });
  const lossy = $derived(format !== 'png');
  const fullPath = $derived(dir && name ? joinPath(dir, name, format) : '');

  onMount(async () => {
    info = await saveInfo();
    dir = info.dir;
    name = info.name;
    requestAnimationFrame(() => nameInput?.select());
  });

  // 预览：把选区画进 380×300 以内，保持比例
  $effect(() => {
    if (!preview || !img.naturalWidth) return;
    const k = Math.min(380 / sel.width, 300 / sel.height, 1);
    const dpr = devicePixelRatio || 1;
    const w = Math.max(1, Math.round(sel.width * k));
    const h = Math.max(1, Math.round(sel.height * k));
    preview.style.width = `${w}px`;
    preview.style.height = `${h}px`;
    preview.width = Math.round(w * dpr);
    preview.height = Math.round(h * dpr);
    const g = preview.getContext('2d')!;
    g.imageSmoothingQuality = 'high';
    g.drawImage(img, sel.x, sel.y, sel.width, sel.height, 0, 0, preview.width, preview.height);
  });

  // 预计大小：参数停 250ms 后真实编码一次
  let estSeq = 0;
  $effect(() => {
    const out = { ...output };
    const seq = ++estSeq;
    bytes = null;
    const t = setTimeout(async () => {
      try {
        const n = await captureEstimate(index, sel, out);
        if (seq === estSeq) bytes = n;
      } catch {
        /* 截图已结束 */
      }
    }, 250);
    return () => clearTimeout(t);
  });

  function setW(v: number) {
    if (lock) ({ width, height } = lockedSize(sel, 'width', v));
    else width = Math.max(1, Math.round(v) || 1);
  }
  function setH(v: number) {
    if (lock) ({ width, height } = lockedSize(sel, 'height', v));
    else height = Math.max(1, Math.round(v) || 1);
  }

  /** 记住这次用的格式、质量和目录，下次保存沿用。 */
  function remember() {
    const d = info && dir === info.dir ? undefined : dir;
    updateConfig({ save: { format, quality, ...(d !== undefined ? { dir: d } : {}) } });
  }

  function save() {
    if (!name.trim()) return nameInput?.focus();
    remember();
    onsave(output, { dir, name });
  }

  function system() {
    remember();
    onsystem(output, dir, name);
  }

  async function addLocation() {
    const picked = await pickFolder(dir);
    if (!picked) return;
    dir = picked;
    const extra = config.value.save.extraDirs;
    if (!extra.includes(picked) && !info?.quick.some((q) => q.path === picked)) {
      await updateConfig({ save: { extraDirs: [...extra, picked] } });
      info = await saveInfo();
    }
  }

  function onKey(e: KeyboardEvent) {
    // 吞掉按键，不让编辑态的快捷键（WASD、P、C…）在保存窗口里生效
    e.stopPropagation();
    if (e.key === 'Enter') {
      e.preventDefault();
      save();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      oncancel();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="backdrop" onpointerdown={(e) => e.stopPropagation()} onkeydown={onKey}>
  <div class="dlg" role="dialog" aria-modal="true" aria-label="保存截图" in:popIn out:popOut>
    <header>
      <b>保存截图</b>
      <IconButton icon="close" label="取消  Esc" size={28} onclick={oncancel} />
    </header>
    <div class="body">
      <div class="preview"><canvas bind:this={preview}></canvas></div>
      <div class="form">
        <div class="group">
          <span class="lbl">保存位置</span>
          <div class="line">
            <Field mono bind:value={dir} width="100%" aria-label="保存目录" />
            <IconButton icon="folder" label="打开目录" size={30} onclick={() => openFolder(dir)} />
          </div>
          <div class="chips">
            {#each info?.quick ?? [] as q (q.path)}
              <button type="button" class="qchip" class:on={q.path === dir} title={q.path} onclick={() => (dir = q.path)}>{q.label}</button>
            {/each}
            <button type="button" class="qchip add" title="添加常用位置" onclick={addLocation}><Icon name="plus" size={13} stroke={2} />添加</button>
          </div>
        </div>

        <div class="group">
          <span class="lbl">文件名</span>
          <div class="line">
            <input class="name" bind:this={nameInput} bind:value={name} aria-label="文件名" spellcheck="false" />
            <Seg label="格式" options={FORMATS} bind:value={format} />
          </div>
          <span class="note">{FORMAT_NOTE[format]}</span>
        </div>

        <div class="group">
          <span class="lbl">尺寸</span>
          <div class="line">
            <input class="num" type="number" min="1" value={width} aria-label="宽" onchange={(e) => setW(+e.currentTarget.value)} />
            <IconButton icon={lock ? 'lock' : 'unlock'} label={lock ? '已锁定比例' : '未锁定比例'} on={lock} size={30} onclick={() => (lock = !lock)} />
            <input class="num" type="number" min="1" value={height} aria-label="高" onchange={(e) => setH(+e.currentTarget.value)} />
            <span class="note">原始 {sel.width} × {sel.height}</span>
          </div>
        </div>

        <div class="group" class:dim={!lossy}>
          <span class="lbl">质量 <span class="note">{lossy ? quality : '仅 JPG / WebP'}</span></span>
          <input type="range" min="1" max="100" bind:value={quality} disabled={!lossy} aria-label="质量" />
        </div>

        <div class="est">
          预计大小 <b>{bytes === null ? '…' : formatBytes(bytes)}</b>
          <span class="path" title={fullPath}>{fullPath}</span>
        </div>
      </div>
    </div>
    <footer>
      <Button onclick={system}>系统另存为…</Button>
      <span class="grow"></span>
      <Button onclick={oncancel}>取消</Button>
      <Button variant="primary" icon="download" onclick={save}>保存　Enter</Button>
    </footer>
  </div>
</div>

<style>
  .backdrop {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, .25);
    z-index: 50;
    cursor: default;
  }
  .dlg {
    width: 820px;
    max-width: calc(100vw - 32px);
    border-radius: var(--r-card);
    background: var(--glass2);
    backdrop-filter: var(--blur-2);
    box-shadow: var(--elev);
    color: var(--text);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  header {
    height: 44px;
    display: flex;
    align-items: center;
    padding: 0 8px 0 16px;
    border-bottom: 1px solid var(--line);
  }
  header b { flex: 1; font-size: 13px; }
  .body { display: flex; min-height: 320px; }
  .preview {
    width: 400px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
    background: var(--sunken);
    /* 棋盘格提示边界 */
    background-image: repeating-conic-gradient(rgba(127, 127, 127, .08) 0 25%, transparent 0 50%);
    background-size: 16px 16px;
  }
  .preview canvas { box-shadow: 0 0 0 1px var(--line2), 0 4px 16px rgba(0, 0, 0, .18); }
  .form {
    flex: 1;
    min-width: 0;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .group { display: flex; flex-direction: column; gap: 6px; }
  .group.dim { opacity: .55; }
  .lbl { font-size: 12px; font-weight: 600; }
  .note { font-size: 11.5px; color: var(--text3); font-weight: 400; }
  .line { display: flex; align-items: center; gap: 8px; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .qchip {
    height: 26px;
    padding: 0 10px;
    border-radius: 7px;
    border: 1px solid var(--line2);
    background: transparent;
    font-size: 12px;
    color: var(--text2);
    cursor: pointer;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    transition: background-color var(--d-hover) var(--ease-hover), border-color var(--d-hover);
  }
  .qchip:hover { background: var(--hover); }
  .qchip.on { border-color: var(--accent); background: var(--accent-soft); color: var(--accent); }
  .name, .num {
    height: 34px;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line2);
    background: var(--field);
    color: var(--text);
    font-size: 13px;
    padding: 0 10px;
    min-width: 0;
    transition: border-color var(--d-hover), box-shadow var(--d-hover);
  }
  .name { flex: 1; }
  .num { width: 96px; font-variant-numeric: tabular-nums; }
  .name:focus, .num:focus { outline: none; border-color: var(--accent); box-shadow: 0 0 0 3px var(--accent-soft); }
  input[type='range'] { accent-color: var(--accent); width: 100%; }
  .est {
    margin-top: auto;
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: 12px;
    color: var(--text2);
  }
  .est b { color: var(--text); font-variant-numeric: tabular-nums; }
  .path {
    margin-left: auto;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text3);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
    border-top: 1px solid var(--line);
  }
  .grow { flex: 1; }
</style>
