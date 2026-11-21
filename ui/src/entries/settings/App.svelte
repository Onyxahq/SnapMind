<script lang="ts">
  // 设置窗口。完整的分页布局在 M10.1 实现；这里先放已经用得上的几项：外观、截图。
  import { onMount } from 'svelte';
  import { appInfo, type AppInfo } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import type { ThemeMode } from '$lib/theme';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import AccentPicker from '$lib/ui/AccentPicker.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import ToastHost from '$lib/ui/ToastHost.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  let info = $state<AppInfo | null>(null);
  onMount(async () => {
    info = await appInfo();
  });

  const THEMES: { value: ThemeMode; label: string }[] = [
    { value: 'system', label: '跟随系统' },
    { value: 'light', label: '浅色' },
    { value: 'dark', label: '深夜' },
  ];

  async function save(patch: Parameters<typeof updateConfig>[0]) {
    try {
      await updateConfig(patch);
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }
</script>

<main>
  <h1 class="t-title">外观</h1>

  <Card>
    <Row title="明暗模式" desc="跟随系统时，Windows 切换深浅色会立即生效">
      <Seg
        label="明暗模式"
        options={THEMES}
        value={config.value.appearance.theme}
        onchange={(theme) => save({ appearance: { theme } })}
      />
    </Row>
    <Row title="强调色" desc="主按钮、选区边框和选中状态的颜色">
      <AccentPicker
        value={config.value.appearance.accent}
        onchange={(accent) => save({ appearance: { accent } })}
      />
    </Row>
  </Card>

  <h1 class="t-title">截图</h1>
  <Card>
    <Row title="截图时显示操作提示" desc="截图时按 H 也可以切换，会记住">
      <Switch
        label="截图时显示操作提示"
        checked={config.value.capture.showHints}
        onchange={(showHints) => save({ capture: { showHints } })}
      />
    </Row>
  </Card>

  {#if info}
    <p class="t-note foot">SnapMind v{info.version}</p>
  {/if}
</main>

<ToastHost />

<style>
  main {
    max-width: 720px;
    margin: 0 auto;
    padding: 32px 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  h1 { margin: 0; }
  .foot { margin: 8px 0 0; color: var(--text3); }
</style>
