<script lang="ts">
  import { t, msg } from '$lib/i18n.svelte';
  // 新手引导（M11.2）：第一次运行时打开。讲清楚三件事：SnapMind 待在托盘里、按哪些快捷键、
  // 本地识别模型要下载一次；大模型是可选的。「关于」页可以再打开这一页。
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { inTauri } from '$lib/ipc';
  import { config } from '$lib/config.svelte';
  import { GLOBAL_HOTKEYS, displayKeys, appKeys } from '$lib/hotkey';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Button from '$lib/ui/Button.svelte';
  import OcrModel from './OcrModel.svelte';

  let { go }: { go: (page: 'keys' | 'prov' | 'ocr') => void } = $props();

  /** 截图编辑态里最常用的几个键 */
  const IN_CAPTURE: [string, string][] = [
    [msg('复制'), 'Ctrl+C'],
    [msg('贴到屏幕'), 'Ctrl+T'],
    [msg('识别文字'), 'Shift+C'],
    [msg('翻译'), 'Ctrl+Q'],
    [msg('标注'), msg('空格')],
    [msg('退出'), 'Esc'],
  ];

  function start() {
    if (inTauri()) getCurrentWindow().close();
    else go('ocr');
  }
</script>

<div class="hero">
  <span class="logo" aria-hidden="true">
    <svg viewBox="0 0 24 24"><path d="M4 8V5.5A1.5 1.5 0 015.5 4H8M16 4h2.5A1.5 1.5 0 0120 5.5V8M20 16v2.5a1.5 1.5 0 01-1.5 1.5H16M8 20H5.5A1.5 1.5 0 014 18.5V16M8 10h8M8 14h5" /></svg>
  </span>
  <h1>{t('欢迎使用 SnapMind')}</h1>
  <p>{t('它待在任务栏右下角的托盘里，平时不占地方。按下快捷键就能截图、识别文字和翻译；右键托盘图标可以打开设置或退出。')}</p>
</div>

<div class="step" style:--i="0">
  <Card title={t('随时按快捷键')}>
    {#each GLOBAL_HOTKEYS as g (g.action)}
      <Row title={t(g.name)} desc={t(g.desc)}>
        <span class="keys">
          {#each displayKeys(config.value.hotkeys[g.action]).split(' + ') as k, i (i)}
            {#if i}<i>+</i>{/if}<kbd>{k}</kbd>
          {/each}
        </span>
      </Row>
    {/each}
    <div class="local">
      <span class="lt">{t('截图时')}</span>
      {#each IN_CAPTURE as [name, key] (name)}
        <span class="chip"><kbd>{appKeys(t(key))}</kbd>{t(name)}</span>
      {/each}
      <button type="button" class="link" onclick={() => go('keys')}>{t('全部快捷键 / 修改')}</button>
    </div>
  </Card>
</div>

<div class="step" style:--i="1">
  <Card title={t('下载本地识别模型')}>
    <OcrModel />
  </Card>
</div>

<div class="step" style:--i="2">
  <Card title={t('大模型（可选）')}>
    <Row title={t('让大模型来识别和翻译')} desc={t('复杂版面、手写、公式交给视觉大模型效果更好。添加服务商并填写 Key 后，在「识别与翻译」里安排顺序')}>
      <Button onclick={() => go('prov')}>{t('添加服务商')}</Button>
    </Row>
  </Card>
</div>

<div class="foot step" style:--i="3">
  <Button variant="primary" onclick={start}>{t('开始使用')}</Button>
</div>

<style>
  .hero { display: flex; flex-direction: column; align-items: flex-start; gap: 10px; padding: 12px 0 6px; }
  .logo {
    width: 52px; height: 52px; border-radius: 15px;
    background: var(--accent); color: var(--on-accent, #fff);
    display: inline-flex; align-items: center; justify-content: center;
    animation: pop 320ms cubic-bezier(.2, .9, .3, 1.25) both;
  }
  .logo svg { width: 28px; height: 28px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  h1 { margin: 4px 0 0; font-size: 28px; line-height: 38px; font-weight: 600; }
  .hero p { margin: 0; max-width: 600px; font-size: 14px; line-height: 22px; color: var(--text2); }
  .step { animation: up var(--d-in) var(--ease-in) both; animation-delay: calc(60ms + var(--i) * 50ms); }
  @keyframes up { from { opacity: 0; transform: translateY(calc(8px * var(--m))); } }
  @keyframes pop { from { opacity: 0; transform: scale(calc(1 - .3 * var(--m))); } }
  .keys { display: inline-flex; align-items: center; gap: 4px; flex: none; }
  .keys i { font-style: normal; font-size: 12px; color: var(--text3); }
  kbd {
    font: inherit; font-size: 12px; line-height: 22px; min-width: 14px; padding: 0 7px; text-align: center;
    border-radius: 6px; background: var(--sunken); color: var(--text); box-shadow: inset 0 -1px 0 var(--line2);
  }
  .local { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 12px 16px; border-top: 1px solid var(--line); }
  .lt { font-size: 13px; font-weight: 600; margin-right: 4px; }
  .chip { display: inline-flex; align-items: center; gap: 6px; font-size: 12px; color: var(--text2); }
  .chip kbd { line-height: 20px; padding: 0 6px; }
  .link {
    margin-left: auto; border: 0; background: none; padding: 0; cursor: pointer;
    font: inherit; font-size: 13px; color: var(--accent);
  }
  .link:hover { text-decoration: underline; }
  .foot { display: flex; justify-content: flex-end; padding-top: 4px; }
</style>
