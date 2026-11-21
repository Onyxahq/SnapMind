<script lang="ts">
  // 组件演示页：同一份标记分别套用浅色和深夜令牌，对照 design/prototype/Spec.dc.html 第 5 节。
  // 只在开发服务器里可见（不进生产包）：http://localhost:1420/gallery/index.html
  import Button from '$lib/ui/Button.svelte';
  import IconButton from '$lib/ui/IconButton.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Field from '$lib/ui/Field.svelte';
  import Select from '$lib/ui/Select.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import ToastHost from '$lib/ui/ToastHost.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import AccentPicker from '$lib/ui/AccentPicker.svelte';
  import type { Accent } from '$lib/theme';

  let accent = $state<Accent>('blue');
  let view = $state<'card' | 'overlay'>('card');
  let on = $state(true);
  let off = $state(false);
  let tpl = $state('SnapMind_{yyyyMMdd}');
  let lang = $state('zh');
  let fmt = $state<'hex' | 'rgb' | 'hsl'>('hex');
  let tool = $state('arrow');
  let n = 0;
</script>

<div class="page">
  <header>
    <div>
      <span class="t-label">MICA · COMPONENTS</span>
      <h1 class="t-title">组件演示</h1>
    </div>
    <AccentPicker bind:value={accent} />
  </header>

  <div class="modes">
    {#each [{ id: 'light', name: '浅色' }, { id: 'dark', name: '深夜' }] as m (m.id)}
      <section class="mode" data-theme={m.id} data-accent={accent}>
        <span class="name">{m.name}</span>

        <div class="line">
          <Button variant="primary" icon="translate" onclick={() => toast(`已复制 ${(n += 24)} 字`)}>翻译</Button>
          <Button onclick={() => toast('已保存到 图片\\SnapMind', { kind: 'info' })}>复制</Button>
          <Button variant="primary" disabled>禁用</Button>
          <Button variant="danger" onclick={() => toast('接口超时，已切换到 DeepL', { kind: 'error' })}>清空</Button>
          <IconButton icon="rect" label="矩形" on={tool === 'rect'} onclick={() => (tool = 'rect')} />
          <IconButton icon="arrow" label="箭头" on={tool === 'arrow'} onclick={() => (tool = 'arrow')} />
        </div>

        <div class="line">
          <Seg
            label="呈现方式"
            options={[{ value: 'card', label: '卡片' }, { value: 'overlay', label: '覆盖' }]}
            bind:value={view}
          />
          <Seg
            label="颜色格式"
            options={[{ value: 'hex', label: 'HEX' }, { value: 'rgb', label: 'RGB' }, { value: 'hsl', label: 'HSL' }]}
            bind:value={fmt}
          />
          <Switch label="开" bind:checked={on} />
          <Switch label="关" bind:checked={off} />
          <Switch label="禁用" checked disabled />
          <span class="k">Ctrl+Shift+O</span>
          <span class="tag">EN → 中文</span>
        </div>

        <div class="line">
          <Field mono bind:value={tpl} width="200px" aria-label="文件名模板" />
          <Field placeholder="搜索历史…" width="160px" aria-label="搜索" />
          <Select
            aria-label="目标语言"
            width="140px"
            options={[{ value: 'zh', label: '简体中文' }, { value: 'en', label: 'English' }, { value: 'ja', label: '日本語' }]}
            bind:value={lang}
          />
        </div>

        <div class="line">
          <div class="toast-sample"><span class="badge">✓</span>已复制 96 字</div>
          <span class="chip">624 × 76</span>
        </div>

        <Card title="截图">
          <Row title="截图时显示操作提示" desc="按 H 也可以切换，会记住">
            <Switch label="显示操作提示" bind:checked={on} />
          </Row>
          <Row title="Ctrl+S 的行为" desc="SnapMind 保存窗口 / 系统另存为 / 直接保存">
            <Select
              aria-label="Ctrl+S 的行为"
              options={[{ value: 'zh', label: '保存窗口' }, { value: 'en', label: '系统另存为' }, { value: 'ja', label: '直接保存' }]}
              bind:value={lang}
            />
          </Row>
        </Card>

        <div class="types">
          <span class="t-trans">大语言模型如今可以在翻译文本的同时，完整保留格式。</span>
          <span class="t-src">Large language models can now translate text while keeping formatting intact.</span>
          <span class="t-note">本地 OCR → 大模型 · 0.8 s</span>
          <span class="t-label">原文　译文</span>
        </div>
      </section>
    {/each}
  </div>
</div>

<ToastHost />

<style>
  .page {
    padding: 40px;
    display: flex;
    flex-direction: column;
    gap: 24px;
    min-height: 100vh;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
  }
  h1 { margin: 4px 0 0; }
  .modes {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(520px, 1fr));
    gap: 16px;
  }
  .mode {
    border-radius: var(--r-card);
    background: var(--bg);
    color: var(--text);
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    box-shadow: var(--card-ring);
  }
  .name { font-size: 12px; font-weight: 600; color: var(--text2); }
  .line { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
  .k {
    font-size: 11px;
    padding: 2px 6px;
    border-radius: 5px;
    background: var(--sunken);
    border: 1px solid var(--line2);
  }
  .tag {
    font-size: 12px;
    padding: 4px 8px;
    border-radius: var(--r-tag);
    background: var(--sunken);
    color: var(--text2);
    font-weight: 500;
  }
  .toast-sample {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 40px;
    padding: 0 14px 0 10px;
    border-radius: var(--r-panel);
    background: var(--glass);
    box-shadow: var(--elev);
    font-size: 13px;
  }
  .badge {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 12px;
    font-weight: 700;
  }
  .chip {
    font-size: 12px;
    line-height: 18px;
    padding: 3px 8px;
    border-radius: var(--r-tag);
    background: var(--chip);
    color: var(--chip-text);
  }
  .types { display: flex; flex-direction: column; gap: 6px; }
</style>
