<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  // 设置 → 识别与翻译：上面一张流程卡片实时显示截图翻译会怎么走，下面是一步到位、备用链和翻译偏好。
  import { onMount } from 'svelte';
  import { languages, type Lang } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import { refLabel } from '$lib/chains';
  import { parseGlossary } from '$lib/task';
  import { toast } from '$lib/ui/toast.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Select from '$lib/ui/Select.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import Field from '$lib/ui/Field.svelte';
  import Head from './Head.svelte';
  import Chains from './Chains.svelte';
  import OcrModel from './OcrModel.svelte';

  let langs = $state<Lang[]>([]);
  onMount(async () => {
    langs = await languages();
  });

  async function save(patch: Parameters<typeof updateConfig>[0]) {
    try {
      await updateConfig(patch);
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  const c = $derived(config.value);
  const label = (r: Parameters<typeof refLabel>[0]) => refLabel(r, c.providers);
  const target = $derived(t(langs.find((l) => l.code === c.translation.targetLang)?.name ?? c.translation.targetLang));
  /** 流程：截图 → 每一步实际首选的后端 → 输出 */
  const flow = $derived.by(() => {
    const ch = c.chains;
    if (c.translation.oneshot && ch.oneshot.length) {
      return {
        steps: [label(ch.oneshot[0])],
        out: t('原文 + {target}译文', { target }),
        note:
          t('视觉模型看图直接给出原文和译文，只调用一次。') +
          (ch.fallback && ch.oneshot.length > 1 ? t('失败时依次换 {names}。', { names: ch.oneshot.slice(1).map(label).join(t('、')) }) : ''),
      };
    }
    const rec = ch.recognize[0];
    const tr = ch.translate[0];
    const steps = [rec ? label(rec) : t('（没有识别后端）'), tr ? label(tr) : t('（没有翻译后端）')];
    const spare = ch.fallback ? ch.recognize.length - 1 + ch.translate.length - 1 : 0;
    return {
      steps,
      out: t('{target}译文', { target }),
      note: [
        c.translation.oneshot && !ch.oneshot.length ? t('一步到位已打开，但「备用链 · 一步到位」里还没有视觉模型，先按「识别 → 翻译」走。') : '',
        !tr ? t('翻译链是空的：在下方「备用链 · 翻译」里添加一个大模型或机器翻译服务。') : '',
        spare > 0 ? t('首选失败、超时或返回空结果时，自动换备用（共 {spare} 个备用）。', { spare }) : ch.fallback ? '' : t('自动切换已关闭：只用每条链的第 1 个。'),
      ]
        .filter(Boolean)
        .join(''),
    };
  });
</script>

<Head title={t('识别与翻译')} desc={t('决定截图里的文字怎么被识别、怎么被翻译。下方流程会实时显示结果。')} />

<div class="card flow">
  <div class="chips">
    <span class="chip">{t('截图')}</span>
    {#each flow.steps as s, i (i + s)}
      <svg class="arrow" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
      <span class="chip acc">{s}</span>
    {/each}
    <svg class="arrow" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
    <span class="chip">{flow.out}</span>
  </div>
  {#if flow.note}<span class="note">{flow.note}</span>{/if}
</div>

<Card>
  <OcrModel />
  <Row title={t('表格识别结果')} desc={t('打开时是只看的表格，还是直接可以改单元格；在表格卡片上点笔切换也会记住')}>
    <Seg
      label={t('表格识别结果')}
      options={[
        { value: 'preview', label: t('预览') },
        { value: 'edit', label: t('编辑') },
      ]}
      value={c.capture.tableMode ?? 'preview'}
      onchange={(tableMode) => save({ capture: { tableMode } })}
    />
  </Row>
  <Row title={t('单元格最大宽度')} desc={t('展开整张表格时，内容比这还宽的单元格自动换行')}>
    <Select
      aria-label={t('单元格最大宽度')}
      width="140px"
      options={[...new Set([160, 240, 320, 480, 640, c.capture.tableCellWidth ?? 320])]
        .filter((n) => n > 0)
        .sort((a, b) => a - b)
        .map((n) => ({ value: String(n), label: t('{n} 像素', { n }) }))
        .concat({ value: '0', label: t('不限制') })}
      value={String(c.capture.tableCellWidth ?? 320)}
      onchange={(e) => save({ capture: { tableCellWidth: Number(e.currentTarget.value) } })}
    />
  </Row>
</Card>

<Card>
  <Row title={t('一步到位')} desc={t('识别和翻译交给同一个视觉大模型，看图直接给出原文和译文，只调用一次；需要在「备用链 · 一步到位」里加视觉模型')}>
    <Switch label={t('一步到位')} checked={c.translation.oneshot} onchange={(oneshot) => save({ translation: { oneshot } })} />
  </Row>
</Card>

<Chains />

<Card title={t('翻译偏好')}>
  <Row title={t('目标语言')} desc={t('截图翻译和划词翻译把文字翻译成这种语言')}>
    <Select
      aria-label={t('目标语言')}
      width="180px"
      options={langs.map((l) => ({ value: l.code, label: t(l.name) }))}
      value={c.translation.targetLang}
      onchange={(e) => save({ translation: { targetLang: e.currentTarget.value } })}
    />
  </Row>
  <Row title={t('原文已是目标语言时')} desc={t('能确定原文语言时才生效（中日韩、俄语、英语等）')}>
    <Select
      aria-label={t('原文已是目标语言时')}
      width="180px"
      options={[
        ...langs.filter((l) => l.code !== c.translation.targetLang).map((l) => ({ value: l.code, label: t('翻译成 {name}', { name: t(l.name) }) })),
        { value: 'skip', label: t('不翻译') },
      ]}
      value={c.translation.sameLang === 'skip' ? 'skip' : c.translation.secondLang}
      onchange={(e) => {
        const v = e.currentTarget.value;
        save({ translation: v === 'skip' ? { sameLang: 'skip' } : { sameLang: 'second', secondLang: v } });
      }}
    />
  </Row>
  <Row title={t('双语对照')} desc={t('复制和贴图时，原文和译文按段落交替排列')}>
    <Switch label={t('双语对照')} checked={c.translation.bilingual} onchange={(bilingual) => save({ translation: { bilingual } })} />
  </Row>
  <Row title={t('保留格式')} desc={t('换行、列表、代码块、Markdown 保持不变（大模型翻译时生效）')}>
    <Switch label={t('保留格式')} checked={c.translation.keepFormat} onchange={(keepFormat) => save({ translation: { keepFormat } })} />
  </Row>
  <Row title={t('自动判断专业术语')} desc={t('由大模型识别专有名词和行业术语，保留原文或用通行译法，并在全文统一（大模型翻译时生效）')}>
    <Switch label={t('自动判断专业术语')} checked={c.translation.autoTerms} onchange={(autoTerms) => save({ translation: { autoTerms } })} />
  </Row>
  <Row title={t('不翻译列表')} desc={t('这些词保持原样，用逗号或顿号分隔（大模型翻译时生效）')}>
    <Field
      width="240px"
      aria-label={t('不翻译列表')}
      placeholder={t('例如：SnapMind、Rust')}
      value={c.translation.glossary.join('、')}
      onchange={(e) => save({ translation: { glossary: parseGlossary(e.currentTarget.value) } })}
    />
  </Row>
</Card>

<style>
  .card { border-radius: var(--r-panel); background: var(--card); box-shadow: var(--card-ring); }
  .flow { padding: 14px 16px; display: flex; flex-direction: column; gap: 10px; }
  .chips { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; }
  .chip {
    font-size: 12px; line-height: 24px; padding: 0 10px; border-radius: 7px;
    background: var(--sunken); color: var(--text2); white-space: nowrap; max-width: 260px; overflow: hidden; text-overflow: ellipsis;
  }
  .chip.acc { background: var(--accent-soft); color: var(--accent); font-weight: 600; animation: pop var(--d-in) var(--ease-in) both; }
  @keyframes pop { from { opacity: 0; transform: scale(calc(1 - .06 * var(--m))); } }
  .arrow { width: 14px; height: 14px; fill: none; stroke: var(--text3); stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  .note { font-size: 12px; line-height: 18px; color: var(--text2); }
</style>
