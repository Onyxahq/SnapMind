<script lang="ts">
  // 设置 → 关于：版本、检查更新（M11.3）、开机自动启动（和托盘菜单里的勾选同步）、新手引导、日志。
  import { onMount } from 'svelte';
  import {
    appInfo,
    autostartGet,
    autostartSet,
    logOpen,
    onUpdate,
    updateCheck,
    updateInstall,
    updateStatus,
    type AppInfo,
    type UpdateStatus,
  } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Head from './Head.svelte';

  let { go }: { go: (page: 'welcome') => void } = $props();

  let info = $state<AppInfo | null>(null);
  let autostart = $state(false);
  let up = $state<UpdateStatus | null>(null);
  onMount(() => {
    appInfo().then((i) => (info = i));
    autostartGet().then((a) => (autostart = a));
    updateStatus().then((s) => (up = s));
    const off = onUpdate((s) => (up = s));
    return () => {
      off.then((f) => f());
    };
  });

  const mb = (n: number) => (n / 1048576).toFixed(1);
  const pct = $derived(up && up.total ? Math.min(100, (up.received / up.total) * 100) : 0);
  const hhmm = (t: number) => new Date(t).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  const upDesc = $derived.by(() => {
    if (!up) return '';
    if (!up.configured) return '这个版本没有配置更新源，新版本请到发布页下载';
    switch (up.phase) {
      case 'checking':
        return '正在检查…';
      case 'latest':
        return `已是最新版本${up.checkedAt ? `（${hhmm(up.checkedAt)} 检查）` : ''}`;
      case 'error':
        return `检查失败：${up.error ?? '未知原因'}`;
      case 'downloading':
        return '正在下载新版本…';
      case 'installing':
        return '正在打开安装程序，装好后 SnapMind 会自动重新打开';
      default:
        return `当前版本 ${up.current}`;
    }
  });

  async function check() {
    up = await updateCheck();
  }
  function install() {
    // 失败原因随状态广播回来
    updateInstall().catch(() => {});
  }
</script>

<Head title="关于" />

<Card>
  <Row title="SnapMind {info?.version ?? ''}" desc="截图 · 识别 · 翻译。本地 OCR 基于 PaddleOCR（PP-OCRv5），界面基于 Tauri 2 和 Svelte 5。" />
  <Row
    title="开源组件与许可"
    desc="本地识别模型 PP-OCRv5 和 SLANet_plus 由 PaddlePaddle 以 Apache-2.0 发布（可商用），首次使用时从官方仓库下载；推理引擎 ONNX Runtime（MIT）；Tauri、Svelte、KaTeX（MIT / Apache-2.0）；模型能力目录数据来自 models.dev（MIT）。"
  />
</Card>

<Card title="更新">
  {#if up?.phase === 'available' || up?.phase === 'downloading' || up?.phase === 'installing'}
    <div class="new">
      <div class="nt">
        <b>新版本 {up.version}</b>
        <span>{up.phase === 'available' ? `当前 ${up.current}。点「下载并安装」后会退出 SnapMind，装好自动重新打开（开着的贴图会先存进历史）` : upDesc}</span>
      </div>
      {#if up.phase === 'available'}
        <Button variant="primary" icon="download" onclick={install}>下载并安装</Button>
      {:else if up.phase === 'downloading'}
        <div class="dl" role="progressbar" aria-label="下载进度" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(pct)}>
          <div class="bar"><i style:width="{pct}%"></i></div>
          <span>{up.total ? `${mb(up.received)} / ${mb(up.total)} MB` : `${mb(up.received)} MB`}</span>
        </div>
      {/if}
    </div>
    {#if up.notes}<p class="notes">{up.notes}</p>{/if}
  {:else}
    <Row title="检查更新" desc={upDesc}>
      {#if up?.configured}
        <Button icon="refresh" disabled={up.phase === 'checking'} onclick={check}>{up.phase === 'checking' ? '检查中…' : '检查更新'}</Button>
      {/if}
    </Row>
  {/if}
  <Row title="自动检查更新" desc="启动后和之后每天在后台检查一次；有新版本时托盘菜单会提示，不会自动安装">
    <Switch
      label="自动检查更新"
      checked={config.value.update.autoCheck}
      onchange={(autoCheck) => updateConfig({ update: { autoCheck } }).catch((e) => toast(String(e), { kind: 'error' }))}
    />
  </Row>
</Card>

<Card>
  <Row title="开机自动启动" desc="开机后在托盘里待命，按快捷键就能截图；托盘菜单里也有同一个开关">
    <Switch label="开机自动启动" checked={autostart} onchange={async (on) => (autostart = await autostartSet(on))} />
  </Row>
  <Row title="新手引导" desc="常用快捷键和本地识别模型下载">
    <Button onclick={() => go('welcome')}>打开</Button>
  </Row>
  <Row title="日志" desc="出错和崩溃的记录。遇到问题时可以把日志文件夹里的 snapmind.log 发给开发者（不含截图和识别出的文字）">
    <Button icon="folder" onclick={() => logOpen().catch((e) => toast(String(e), { kind: 'error' }))}>打开日志文件夹</Button>
  </Row>
</Card>

<style>
  .new { display: flex; align-items: center; gap: 16px; padding: 12px 16px; animation: in var(--d-in) var(--ease-in) both; }
  @keyframes in { from { opacity: 0; transform: translateY(calc(4px * var(--m))); } }
  .nt { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .nt b { font-size: 14px; color: var(--accent); }
  .nt span { font-size: 12px; line-height: 18px; color: var(--text2); }
  .notes {
    margin: 0; padding: 10px 16px 12px; border-top: 1px solid var(--line);
    font-size: 12px; line-height: 19px; color: var(--text2); white-space: pre-line;
    max-height: 160px; overflow-y: auto;
  }
  .dl { display: flex; flex-direction: column; align-items: flex-end; gap: 6px; width: 180px; flex: none; }
  .dl span { font-size: 12px; color: var(--text2); font-variant-numeric: tabular-nums; }
  .bar { width: 100%; height: 6px; border-radius: 3px; background: var(--sunken); overflow: hidden; }
  .bar i { display: block; height: 100%; border-radius: 3px; background: var(--accent); transition: width 160ms linear; }
</style>
