<script lang="ts">
  import { t, msg } from '$lib/i18n.svelte';
  // 设置 → 外观：明暗模式（三张预览卡片）、强调色、悬浮球（M10.5）。切换立即生效，已打开的窗口一起刷新。
  import { config, updateConfig } from '$lib/config.svelte';
  import type { ThemeMode } from '$lib/theme';
  import { toast } from '$lib/ui/toast.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import AccentPicker from '$lib/ui/AccentPicker.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import Head from './Head.svelte';

  /** 预览卡片：左半边和右半边（跟随系统时一半浅色一半深色） */
  const LIGHT = { bg: '#eef1f6', card: '#ffffff', line: '#cfd5df' };
  const DARK = { bg: '#11141b', card: '#232835', line: '#4a5163' };
  const MODES: { value: ThemeMode; label: string; a: typeof LIGHT; b: typeof LIGHT }[] = [
    { value: 'system', label: msg('跟随系统'), a: LIGHT, b: DARK },
    { value: 'light', label: msg('浅色'), a: LIGHT, b: LIGHT },
    { value: 'dark', label: msg('深夜'), a: DARK, b: DARK },
  ];

  async function save(patch: Parameters<typeof updateConfig>[0]) {
    try {
      await updateConfig(patch);
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }
</script>

<Head title={t('外观')} desc={t('切换立即生效，已打开的贴图和卡片会一起刷新。')} />

<div class="tiles" role="radiogroup" aria-label={t('明暗模式')}>
  {#each MODES as m (m.value)}
    {@const on = config.value.appearance.theme === m.value}
    <button type="button" class="tile" class:on role="radio" aria-checked={on} onclick={() => save({ appearance: { theme: m.value } })}>
      <span class="shot">
        {#each [m.a, m.b] as p, i (i)}
          <span class="half" style:background={p.bg}>
            <span class="mini" style:background={p.card}>
              <span class="l1" style:background={p.line}></span>
              <span class="l2"></span>
            </span>
          </span>
        {/each}
      </span>
      <b>{t(m.label)}</b>
    </button>
  {/each}
</div>

<Card>
  <Row title={t('强调色')} desc={t('主按钮、选区边框和选中状态的颜色；深夜模式下自动换成同色相的提亮版本')}>
    <AccentPicker value={config.value.appearance.accent} onchange={(accent) => save({ appearance: { accent } })} />
  </Row>
  <Row title={t('界面动效')} desc={t('「减弱」只保留淡入淡出，去掉位移、缩放和列表重排；系统关掉了「显示动画」时也会自动减弱')}>
    <Seg
      label={t('界面动效')}
      options={[
        { value: 'full', label: t('完整') },
        { value: 'reduced', label: t('减弱') },
      ]}
      value={config.value.appearance.motion}
      onchange={(motion: 'full' | 'reduced') => save({ appearance: { motion } })}
    />
  </Row>
  <Row title={t('悬浮球')} desc={t('屏幕边缘的快捷入口：单击截图，右键更多，可拖动，贴边半隐藏；截图和录屏里不会出现')}>
    <Switch label={t('悬浮球')} checked={config.value.ball.enabled} onchange={(enabled) => save({ ball: { enabled } })} />
  </Row>
</Card>

<style>
  .tiles { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; }
  .tile {
    display: flex; flex-direction: column; gap: 10px; padding: 10px;
    border: 0; border-radius: var(--r-panel); background: var(--card); box-shadow: var(--card-ring);
    color: var(--text); font: inherit; text-align: left; cursor: pointer;
    transition: box-shadow var(--d-hover) var(--ease-hover), transform var(--d-press, 90ms) var(--ease-hover);
  }
  .tile:hover { box-shadow: var(--card-ring), 0 0 0 1px var(--line2); }
  .tile:active { transform: scale(.985); }
  .tile.on { box-shadow: 0 0 0 2px var(--accent); }
  .tile b { font-size: 14px; font-weight: 600; padding: 0 4px; }
  .shot { display: flex; height: 88px; border-radius: 8px; overflow: hidden; box-shadow: 0 0 0 1px var(--line); }
  .half { flex: 1; display: flex; align-items: flex-end; padding: 10px; }
  .mini {
    width: 100%; height: 36px; border-radius: 6px; box-sizing: border-box; padding: 7px;
    display: flex; flex-direction: column; gap: 5px; box-shadow: 0 4px 10px rgba(0, 0, 0, .15);
  }
  .l1 { width: 60%; height: 5px; border-radius: 3px; }
  .l2 { width: 40%; height: 5px; border-radius: 3px; background: var(--accent); }
</style>
