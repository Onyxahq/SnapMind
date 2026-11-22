<script lang="ts">
  // 设置 → 关于：版本、开机自动启动（和托盘菜单里的勾选同步）。检查更新在 M11.3 加入。
  import { onMount } from 'svelte';
  import { appInfo, autostartGet, autostartSet, type AppInfo } from '$lib/ipc';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Head from './Head.svelte';

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
</Card>
