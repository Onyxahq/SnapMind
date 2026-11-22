<script lang="ts">
  // 设置 → 关于：版本、开机自动启动（和托盘菜单里的勾选同步）、新手引导、日志。检查更新在 M11.3 加入。
  import { onMount } from 'svelte';
  import { appInfo, autostartGet, autostartSet, logOpen, type AppInfo } from '$lib/ipc';
  import { toast } from '$lib/ui/toast.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Head from './Head.svelte';

  let { go }: { go: (page: 'welcome') => void } = $props();

  let info = $state<AppInfo | null>(null);
  let autostart = $state(false);
  onMount(async () => {
    info = await appInfo();
    autostart = await autostartGet();
  });
</script>

<Head title="关于" />

<Card>
  <Row title="SnapMind {info?.version ?? ''}" desc="截图 · 识别 · 翻译。本地 OCR 基于 PaddleOCR（PP-OCRv5），界面基于 Tauri 2 和 Svelte 5。" />
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
