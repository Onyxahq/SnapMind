<script lang="ts">
  // 设置窗口。完整的分页布局在 M10.1 实现；这里先放已经用得上的几项：外观、截图、模型服务。
  import { onMount } from 'svelte';
  import { appInfo, type AppInfo } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import type { ThemeMode } from '$lib/theme';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import AccentPicker from '$lib/ui/AccentPicker.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Select from '$lib/ui/Select.svelte';
  import Field from '$lib/ui/Field.svelte';
  import type { SaveConfig } from '$lib/ipc';
  import ToastHost from '$lib/ui/ToastHost.svelte';
  import Providers from './Providers.svelte';
  import Chains from './Chains.svelte';
  import Prompts from './Prompts.svelte';
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

  const CTRL_S: { value: SaveConfig['ctrlS']; label: string }[] = [
    { value: 'window', label: 'SnapMind 保存窗口' },
    { value: 'system', label: '系统另存为' },
    { value: 'direct', label: '直接保存' },
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
    <Row title="退出时确认未保存的标注" desc="标注还没复制或保存就按 Esc 退出时，先问一下">
      <Switch
        label="退出时确认未保存的标注"
        checked={config.value.capture.confirmDiscard}
        onchange={(confirmDiscard) => save({ capture: { confirmDiscard } })}
      />
    </Row>
    <Row title="Ctrl+S 的行为" desc="保存窗口可以改名、换格式和尺寸；直接保存会存到默认目录">
      <Select
        aria-label="Ctrl+S 的行为"
        width="160px"
        options={CTRL_S}
        value={config.value.save.ctrlS}
        onchange={(e) => save({ save: { ctrlS: e.currentTarget.value as SaveConfig['ctrlS'] } })}
      />
    </Row>
    <Row title="文件名模板" desc="花括号里写时间格式：yyyy MM dd HH mm ss SSS">
      <Field
        mono
        width="240px"
        aria-label="文件名模板"
        value={config.value.save.filenameTemplate}
        onchange={(e) => save({ save: { filenameTemplate: e.currentTarget.value.trim() || 'SnapMind_{yyyyMMdd_HHmmss}' } })}
      />
    </Row>
  </Card>

  <div class="head">
    <h1 class="t-title">模型服务</h1>
    <p class="t-note">先添加服务商并填写 Key，再勾选要显示的模型。勾选的模型才会出现在下拉框和备用链里。</p>
  </div>
  <Providers />
  <Chains />

  <div class="head">
    <h1 class="t-title">提示词</h1>
    <p class="t-note">只在用到大模型时生效。每种任务一套提示词，可以随时恢复默认。</p>
  </div>
  <Prompts />

  {#if info}
    <p class="t-note foot">SnapMind v{info.version}</p>
  {/if}
</main>

<ToastHost />

<style>
  main {
    max-width: 820px;
    margin: 0 auto;
    padding: 32px 32px 24px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  h1 { margin: 0; }
  .head p { margin: 2px 0 0; }
  .foot { margin: 8px 0 0; color: var(--text3); }
</style>
