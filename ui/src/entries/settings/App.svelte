<script lang="ts">
  // 设置窗口。完整的分页布局在 M10.1 实现；这里先放已经用得上的几项：外观、截图、模型服务。
  import { onMount } from 'svelte';
  import { appInfo, languages, type AppInfo, type Lang } from '$lib/ipc';
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
  import { parseGlossary } from '$lib/task';
  import { toast } from '$lib/ui/toast.svelte';

  let info = $state<AppInfo | null>(null);
  let langs = $state<Lang[]>([]);
  onMount(async () => {
    info = await appInfo();
    langs = await languages();
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

  <h1 class="t-title">翻译</h1>
  <Card>
    <Row title="目标语言" desc="截图翻译（Ctrl+Q、Ctrl+Shift+O）把文字翻译成这种语言">
      <Select
        aria-label="目标语言"
        width="160px"
        options={langs.map((l) => ({ value: l.code, label: l.name }))}
        value={config.value.translation.targetLang}
        onchange={(e) => save({ translation: { targetLang: e.currentTarget.value } })}
      />
    </Row>
    <Row title="原文已是目标语言时" desc="能确定原文语言时才生效（中日韩、俄语、英语等）">
      <Select
        aria-label="原文已是目标语言时"
        width="180px"
        options={[
          ...langs.filter((l) => l.code !== config.value.translation.targetLang).map((l) => ({ value: l.code, label: `翻译成 ${l.name}` })),
          { value: 'skip', label: '不翻译' },
        ]}
        value={config.value.translation.sameLang === 'skip' ? 'skip' : config.value.translation.secondLang}
        onchange={(e) => {
          const v = e.currentTarget.value;
          save({ translation: v === 'skip' ? { sameLang: 'skip' } : { sameLang: 'second', secondLang: v } });
        }}
      />
    </Row>
    <Row title="双语对照" desc="复制和贴图时，原文和译文按段落交替排列">
      <Switch label="双语对照" checked={config.value.translation.bilingual} onchange={(bilingual) => save({ translation: { bilingual } })} />
    </Row>
    <Row title="保留格式" desc="换行、列表、代码块、Markdown 保持不变（大模型翻译时生效）">
      <Switch label="保留格式" checked={config.value.translation.keepFormat} onchange={(keepFormat) => save({ translation: { keepFormat } })} />
    </Row>
    <Row title="自动判断专业术语" desc="由大模型识别专有名词和行业术语，保留原文或用通行译法，并在全文统一（大模型翻译时生效）">
      <Switch label="自动判断专业术语" checked={config.value.translation.autoTerms} onchange={(autoTerms) => save({ translation: { autoTerms } })} />
    </Row>
    <Row title="不翻译列表" desc="这些词保持原样，用逗号或顿号分隔（大模型翻译时生效）">
      <Field
        width="240px"
        aria-label="不翻译列表"
        placeholder="例如：SnapMind、Rust"
        value={config.value.translation.glossary.join('、')}
        onchange={(e) => save({ translation: { glossary: parseGlossary(e.currentTarget.value) } })}
      />
    </Row>
    <Row title="一步到位" desc="识别和翻译交给同一个视觉模型，看图直接给出原文和译文，只调用一次；需要在「备用链 · 一步到位」里加视觉模型">
      <Switch label="一步到位" checked={config.value.translation.oneshot} onchange={(oneshot) => save({ translation: { oneshot } })} />
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
