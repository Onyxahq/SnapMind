<script lang="ts">
  import { t, msg } from '$lib/i18n.svelte';
  // 设置页「提示词」（需求 1.3）：四套任务各一套，空表示用默认值；可插入变量、恢复默认。
  import { onMount, tick } from 'svelte';
  import { promptDefaults, type PromptKind } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import Seg from '$lib/ui/Seg.svelte';
  import Button from '$lib/ui/Button.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  const KINDS: { value: PromptKind; label: string; desc: string }[] = [
    { value: 'recognize', label: msg('识别'), desc: msg('识别链用视觉大模型时生效。本地 OCR 不使用提示词。') },
    { value: 'translate', label: msg('翻译'), desc: msg('翻译链用大模型时生效，划词翻译也用这一套。机器翻译不使用提示词。') },
    { value: 'oneshot', label: msg('一步到位'), desc: msg('打开「一步到位」后生效：一次调用同时完成识别和翻译，要求模型按 JSON 输出。') },
    { value: 'markdown', label: 'Markdown', desc: msg('「识别为 Markdown」使用大模型时生效。') },
  ];
  const VARS = [
    { v: '{target_lang}', tip: msg('目标语言，如「简体中文」') },
    { v: '{source_lang}', tip: msg('原文语言；自动检测时为空') },
    { v: '{glossary}', tip: msg('不翻译列表里的术语，用顿号分隔；为空时为空') },
    { v: '{text}', tip: msg('要翻译的原文') },
  ];

  let kind = $state<PromptKind>('translate');
  let defaults = $state<Record<PromptKind, string> | null>(null);
  let draft = $state('');
  let area = $state<HTMLTextAreaElement>();
  let timer: ReturnType<typeof setTimeout> | undefined;

  const meta = $derived(KINDS.find((k) => k.value === kind)!);
  const custom = $derived(config.value.prompts[kind]);
  const isDefault = $derived(!custom.trim());

  onMount(async () => {
    defaults = await promptDefaults();
    load();
  });

  function load() {
    if (!defaults) return;
    draft = isDefault ? defaults[kind] : custom;
  }

  async function flush() {
    clearTimeout(timer);
    timer = undefined;
    if (!defaults) return;
    // 和默认值一样就存空字符串，以后默认值改进了也能跟着用上
    const value = draft.trim() === defaults[kind].trim() ? '' : draft;
    if (value === config.value.prompts[kind]) return;
    try {
      await updateConfig({ prompts: { [kind]: value } });
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  function oninput() {
    clearTimeout(timer);
    timer = setTimeout(flush, 400);
  }

  async function switchTo(k: PromptKind) {
    await flush();
    kind = k;
    load();
  }

  async function reset() {
    clearTimeout(timer);
    try {
      await updateConfig({ prompts: { [kind]: '' } });
      load();
      toast(t('已恢复默认提示词'), { kind: 'ok' });
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  /** 在光标处插入变量 */
  async function insert(v: string) {
    const el = area;
    if (!el) return;
    const s = el.selectionStart ?? draft.length;
    const e = el.selectionEnd ?? s;
    draft = draft.slice(0, s) + v + draft.slice(e);
    oninput();
    await tick();
    el.focus();
    el.setSelectionRange(s + v.length, s + v.length);
  }
</script>

<div class="bar">
  <Seg label={t('提示词')} options={KINDS} value={kind} onchange={switchTo} />
  <span class="state" class:mod={!isDefault}>{isDefault ? t('默认') : t('已修改')}</span>
  <span class="grow"></span>
  <Button disabled={isDefault} onclick={reset}>{t('恢复默认')}</Button>
</div>

<div class="card">
  <span class="desc">{t(meta.desc)}</span>
  <textarea
    bind:this={area}
    class="field"
    rows="10"
    aria-label={t('提示词')}
    spellcheck="false"
    bind:value={draft}
    {oninput}
    onblur={flush}
  ></textarea>
  <div class="vars">
    <span class="lbl">{t('插入变量')}</span>
    {#each VARS as x (x.v)}
      <button type="button" class="chip" title={t(x.tip)} onclick={() => insert(x.v)}>{x.v}</button>
    {/each}
  </div>
  <span class="hint">{t('某个变量为空时（例如没有不翻译的术语），含这个变量的整行会被去掉；翻译提示词里删掉了 {v} 时，原文会接在最后。', { v: '{text}' })}</span>
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .grow { flex: 1; }
  .state {
    font-size: 12px;
    color: var(--text3);
    transition: color var(--d-hover) var(--ease-hover);
  }
  .state.mod { color: var(--accent); }
  .card {
    background: var(--card);
    border-radius: var(--r-panel);
    box-shadow: var(--card-ring);
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .desc {
    font-size: 12px;
    line-height: 18px;
    color: var(--text2);
  }
  textarea {
    resize: vertical;
    min-height: 180px;
    border-radius: var(--r-ctl);
    border: 1px solid var(--line2);
    background: var(--field);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 20px;
    padding: 10px 12px;
    transition:
      border-color var(--d-hover) var(--ease-hover),
      box-shadow var(--d-hover) var(--ease-hover);
  }
  textarea:hover:not(:focus) { border-color: var(--text3); }
  textarea:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .vars {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .lbl { font-size: 12px; color: var(--text3); }
  .chip {
    font: inherit;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 22px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--r-tag);
    background: var(--chip);
    color: var(--chip-text);
    cursor: pointer;
    transition:
      filter var(--d-hover) var(--ease-hover),
      transform var(--d-press) var(--ease-press);
  }
  .chip:hover { filter: brightness(1.1); }
  .chip:active { transform: scale(.95); }
  .chip:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--accent-soft); }
  .hint {
    font-size: 12px;
    line-height: 18px;
    color: var(--text3);
  }
</style>
