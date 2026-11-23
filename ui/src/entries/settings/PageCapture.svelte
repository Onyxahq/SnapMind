<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 设置 → 截图与贴图：图片格式与质量、文件名模板（实时预览）、保存目录、Ctrl+S 的行为、退出确认；录屏的默认选择。
  import { onMount } from 'svelte';
  import { openFolder, pickFolder, saveInfo, type ImageFormat, type RecordConfig, type SaveConfig } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import { renderFilename } from '$lib/filename';
  import { toast } from '$lib/ui/toast.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Select from '$lib/ui/Select.svelte';
  import Field from '$lib/ui/Field.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Head from './Head.svelte';

  const DEFAULT_TEMPLATE = 'SnapMind_{yyyyMMdd_HHmmss}';
  const s = $derived(config.value.save);
  const r = $derived(config.value.record);
  const lossy = $derived(s.format !== 'png');

  /** 实际的保存目录（配置为空时是「图片\SnapMind」） */
  let dir = $state('');
  let now = $state(new Date());
  async function refreshDir() {
    dir = (await saveInfo()).dir;
  }
  onMount(() => {
    refreshDir();
    const t = setInterval(() => (now = new Date()), 1000);
    return () => clearInterval(t);
  });

  async function save(patch: Parameters<typeof updateConfig>[0]) {
    try {
      await updateConfig(patch);
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  async function changeDir() {
    const picked = await pickFolder(dir);
    if (!picked) return;
    await save({ save: { dir: picked } });
    await refreshDir();
  }

  async function resetDir() {
    await save({ save: { dir: '' } });
    await refreshDir();
  }

  let quality = $state(90);
  let tpl = $state('');
  $effect(() => {
    quality = s.quality;
    tpl = s.filenameTemplate;
  });

  const preview = $derived(`${renderFilename(tpl || DEFAULT_TEMPLATE, now)}.${s.format}`);
  const fpsOptions = $derived(
    (r.format === 'gif' ? [10, 15, 30] : [15, 30, 60]).map((v) => ({ value: String(v), label: `${v} fps${v === 15 && r.format === 'gif' ? t('（推荐）') : ''}` })),
  );

  function setRecord(p: Partial<RecordConfig>) {
    const next = { ...r, ...p };
    if (next.format === 'gif' && next.fps > 30) next.fps = 30;
    save({ record: next });
  }
</script>

<Head title={t('截图与贴图')} desc={t('复制、保存、录屏和贴图的默认行为。')} />

<Card title={t('截图')}>
  <Row title={t('图片格式')} desc={t('保存截图时使用；复制到剪贴板时始终是 PNG（无损）')}>
    <Seg
      label={t('图片格式')}
      options={[
        { value: 'png', label: 'PNG' },
        { value: 'jpg', label: 'JPG' },
        { value: 'webp', label: 'WebP' },
      ]}
      value={s.format}
      onchange={(format: ImageFormat) => save({ save: { format } })}
    />
  </Row>
  <Row title={t('图片质量')} desc={lossy ? t('越高越清晰，文件也越大') : t('PNG 是无损格式，不需要设置质量')}>
    <div class="slider">
      <input
        type="range"
        min="40"
        max="100"
        bind:value={quality}
        disabled={!lossy}
        aria-label={t('图片质量')}
        onchange={() => save({ save: { quality: Number(quality) } })}
      />
      <span class="val">{lossy ? quality : '—'}</span>
    </div>
  </Row>
  <Row title={t('文件名模板')} desc={t('花括号里写时间格式：yyyy MM dd HH mm ss SSS')}>
    <div class="tpl">
      <Field
        mono
        width="280px"
        aria-label={t('文件名模板')}
        value={tpl}
        oninput={(e) => (tpl = e.currentTarget.value)}
        onchange={(e) => save({ save: { filenameTemplate: e.currentTarget.value.trim() || DEFAULT_TEMPLATE } })}
      />
      <span class="pv" title={preview}>{t('预览：{preview}', { preview })}</span>
    </div>
  </Row>
  <Row title={t('保存目录')} desc={dir || '…'}>
    <div class="btns">
      <Button variant="ghost" icon="folder" onclick={() => dir && openFolder(dir)}>{t('打开目录')}</Button>
      <Button variant="ghost" onclick={changeDir}>{t('更改')}</Button>
      {#if s.dir}<Button variant="ghost" onclick={resetDir} title={t('改回「图片\\SnapMind」')}>{t('默认')}</Button>{/if}
    </div>
  </Row>
  <Row title={t('Ctrl+S 保存方式')} desc={t('保存窗口可以改名、换格式和尺寸；直接保存会存到上面的目录')}>
    <Seg
      label={t('Ctrl+S 保存方式')}
      options={[
        { value: 'window', label: t('SnapMind 保存窗口') },
        { value: 'system', label: t('系统另存为') },
        { value: 'direct', label: t('直接保存') },
      ]}
      value={s.ctrlS}
      onchange={(ctrlS: SaveConfig['ctrlS']) => save({ save: { ctrlS } })}
    />
  </Row>
  <Row title={t('有标注时退出前确认')} desc={t('标注还没复制或保存就按 Esc 退出时，先问一下')}>
    <Switch
      label={t('有标注时退出前确认')}
      checked={config.value.capture.confirmDiscard}
      onchange={(confirmDiscard) => save({ capture: { confirmDiscard } })}
    />
  </Row>
</Card>

<Card title={t('录屏')}>
  <Row title={t('默认格式')} desc={t('按 R 时预选的格式，录制前还可以切换')}>
    <Seg
      label={t('默认格式')}
      options={[
        { value: 'mp4', label: t('MP4 视频') },
        { value: 'gif', label: t('GIF 动图') },
      ]}
      value={r.format}
      onchange={(format: RecordConfig['format']) => setRecord({ format })}
    />
  </Row>
  <Row title={t('帧率')}>
    <Select
      aria-label={t('帧率')}
      width="160px"
      options={fpsOptions}
      value={String(r.fps)}
      onchange={(e) => setRecord({ fps: Number(e.currentTarget.value) })}
    />
  </Row>
  <Row title={t('录制系统声音')} desc={t('GIF 不包含声音')}>
    <Switch label={t('录制系统声音')} checked={r.audio} disabled={r.format === 'gif'} onchange={(audio) => setRecord({ audio })} />
  </Row>
</Card>

<style>
  .slider { display: flex; align-items: center; gap: 10px; width: 240px; }
  .slider input { flex: 1; accent-color: var(--accent); }
  .val { width: 32px; text-align: right; font-size: 13px; color: var(--text2); font-variant-numeric: tabular-nums; }
  .tpl { display: flex; flex-direction: column; align-items: flex-end; gap: 6px; }
  .pv { font-size: 12px; color: var(--text3); max-width: 280px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .btns { display: flex; gap: 6px; }
</style>
