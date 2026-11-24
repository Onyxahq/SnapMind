<script lang="ts">
  import { t, msg } from '$lib/i18n.svelte';
  // 设置 → 快捷键：点一下快捷键再按新的组合键即可修改（Esc 取消，Backspace 清除）。
  // 录制期间全局快捷键全部停用，免得按下 Shift+F1 直接开始截图；录完恢复。
  // 和另一项重复时当场提示；被别的程序占用时标红（以系统注册结果为准）。
  import { onMount } from 'svelte';
  import {
    DEFAULT_CONFIG,
    hotkeysStatus,
    hotkeysSuspend,
    type HotkeyAction,
    type HotkeyStatus,
  } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import { GLOBAL_HOTKEYS, displayKeys, record, sameKeys, appKeys } from '$lib/hotkey';
  import { toast } from '$lib/ui/toast.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Head from './Head.svelte';

  const GLOBAL = GLOBAL_HOTKEYS;

  const LOCAL: [string, string][] = [
    [msg('复制并结束'), 'Ctrl+C / Enter'],
    [msg('贴图并结束'), 'Ctrl+T'],
    [msg('保存…'), 'Ctrl+S'],
    [msg('开始 / 退出标注'), msg('空格')],
    [msg('识别文字'), 'Shift+C'],
    [msg('识别为 Markdown'), 'Shift+Alt+R'],
    [msg('翻译'), 'Ctrl+Q'],
    [msg('长截图 / 录屏'), 'L / R'],
    [msg('切换颜色格式'), 'P'],
    [msg('复制颜色值'), 'C'],
    [msg('逐像素移动光标'), 'W A S D'],
    [msg('选择上级 / 下级元素'), msg('滚轮')],
    [msg('标注工具'), '1 – 7'],
    [msg('撤销'), 'Ctrl+Z'],
    [msg('显示 / 隐藏提示'), 'H'],
    [msg('逐级退出'), 'Esc'],
    [msg('贴图：缩放 / 不透明度'), msg('滚轮 / Ctrl+滚轮')],
    [msg('贴图：鼠标穿透'), 'Ctrl+Shift+X'],
  ];

  let status = $state<HotkeyStatus[]>([]);
  /** 正在录制的那一项，以及录制中的提示 */
  let recording = $state<HotkeyAction | null>(null);
  let hint = $state('');
  let error = $state('');

  onMount(() => {
    hotkeysStatus().then((s) => (status = s));
    return () => {
      if (recording) hotkeysSuspend(false);
    };
  });

  const keysOf = (a: HotkeyAction) => config.value.hotkeys[a];
  const statusOf = (a: HotkeyAction) => status.find((s) => s.action === a);

  async function start(a: HotkeyAction) {
    if (recording === a) return stop();
    recording = a;
    hint = '';
    error = '';
    status = await hotkeysSuspend(true);
  }

  async function stop() {
    recording = null;
    hint = '';
    error = '';
    status = await hotkeysSuspend(false);
  }

  async function set(a: HotkeyAction, keys: string) {
    try {
      await updateConfig({ hotkeys: { [a]: keys } });
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
    await stop();
    const st = statusOf(a);
    if (st && !st.ok) toast(`「${GLOBAL.find((g) => g.action === a)?.name}」${st.error}`, { kind: 'error' });
  }

  function onKey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    const r = record(e);
    if (r.kind === 'cancel') return stop();
    if (r.kind === 'clear') return set(recording, '');
    if (r.kind === 'pending') {
      hint = r.hint;
      return;
    }
    const other = GLOBAL.find((g) => g.action !== recording && sameKeys(keysOf(g.action), r.keys));
    if (other) {
      error = t('{displayKeys} 已经用在「{name}」上了，换一个', { displayKeys: displayKeys(r.keys), name: other.name });
      hint = '';
      return;
    }
    set(recording, r.keys);
  }

  async function reset() {
    if (recording) await stop();
    try {
      await updateConfig({ hotkeys: { ...DEFAULT_CONFIG.hotkeys } });
      status = await hotkeysStatus();
      toast(t('已恢复默认快捷键'));
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }
</script>

<svelte:window onkeydown={onKey} onblur={() => recording && stop()} />

<Head title={t('快捷键')} desc={t('点击快捷键后按下新的组合键即可修改；Esc 取消，Backspace 清除（不用这个快捷键）。')}>
  <Button variant="ghost" onclick={reset}>{t('恢复默认')}</Button>
</Head>

<Card title={t('全局')}>
  {#each GLOBAL as g (g.action)}
    {@const st = statusOf(g.action)}
    {@const rec = recording === g.action}
    <Row title={t(g.name)} desc={t(g.desc)}>
      <div class="right">
        {#if rec && (error || hint)}
          <span class="msg" class:bad={!!error}>{error || hint}</span>
        {:else if !rec && st && !st.ok}
          <span class="msg bad" title={st.error ?? ''}>{st.error}</span>
        {/if}
        <button
          type="button"
          class="kbd"
          class:rec
          class:bad={!rec && st && !st.ok}
          class:none={!keysOf(g.action)}
          aria-label={t('{name}的快捷键：{displayKeys}，点击修改', { name: t(g.name), displayKeys: displayKeys(keysOf(g.action)) })}
          onclick={() => start(g.action)}
        >
          {rec ? t('请按下新的组合键…') : displayKeys(keysOf(g.action))}
        </button>
      </div>
    </Row>
  {/each}
</Card>

<Card title={t('截图编辑态与贴图（只在它们打开时生效）')}>
  <div class="grid">
    {#each LOCAL as [name, keys] (name)}
      <div class="lk"><span>{t(name)}</span><span class="k">{appKeys(t(keys))}</span></div>
    {/each}
  </div>
</Card>

<Card>
  <Row title={t('截图时显示操作提示')} desc={t('截图中按 H 也会改变这里，下次截图沿用')}>
    <Switch
      label={t('截图时显示操作提示')}
      checked={config.value.capture.showHints}
      onchange={(showHints) => updateConfig({ capture: { showHints } }).catch((e) => toast(String(e), { kind: 'error' }))}
    />
  </Row>
</Card>

<style>
  .right { display: flex; align-items: center; gap: 10px; }
  .msg { font-size: 12px; color: var(--text3); max-width: 260px; text-align: right; }
  .msg.bad { color: var(--danger); }
  .kbd {
    min-width: 140px;
    height: 32px;
    padding: 0 12px;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line2);
    background: var(--field);
    color: var(--text);
    font: 500 13px/1 var(--font-mono, Consolas, monospace);
    cursor: pointer;
    transition: border-color var(--d-hover) var(--ease-hover), box-shadow var(--d-hover) var(--ease-hover), background var(--d-hover) var(--ease-hover);
  }
  .kbd:hover { border-color: var(--accent); }
  .kbd.rec {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
    color: var(--accent);
    font-family: inherit;
    animation: breathe 1.2s var(--ease-hover) infinite alternate;
  }
  @keyframes breathe { to { box-shadow: 0 0 0 5px var(--accent-soft); } }
  .kbd.bad { border-color: var(--danger); color: var(--danger); }
  .kbd.none { color: var(--text3); font-family: inherit; }
  .grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .lk {
    display: flex; align-items: center; justify-content: space-between; gap: 12px;
    padding: 10px 16px; font-size: 13px; border-top: 1px solid var(--line);
  }
  .lk:nth-child(-n + 2) { border-top: 0; }
  .k {
    font: 12px/20px var(--font-mono, Consolas, monospace); color: var(--text2);
    padding: 0 8px; border-radius: 6px; background: var(--sunken); white-space: nowrap;
  }
</style>
