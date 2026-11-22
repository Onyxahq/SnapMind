<script lang="ts">
  // 设置页「备用链」（需求 1.2）：翻译 / 识别 / 一步到位三条有序链，第 1 个是首选。
  import { flip } from 'svelte/animate';
  import { fade } from 'svelte/transition';
  import type { BackendRef, ChainTask } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import { keyStatus } from '$lib/keys.svelte';
  import { TASKS, available, move, normalize, refKey, refLabel, refState, refTag } from '$lib/chains';
  import Seg from '$lib/ui/Seg.svelte';
  import Select from '$lib/ui/Select.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Button from '$lib/ui/Button.svelte';
  import IconButton from '$lib/ui/IconButton.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  /** 不需要 Key 的服务商种类 */
  const NO_KEY = ['ollama'];
  const TIMEOUTS = [5, 8, 15, 30].map((s) => ({ value: String(s), label: `${s} 秒` }));

  let task = $state<ChainTask>('translate');
  let addSel = $state('');

  const chains = $derived(config.value.chains);
  const chain = $derived(chains[task]);
  const providers = $derived(config.value.providers);
  const cands = $derived(available(task, chain, providers, keyStatus));
  const pick = $derived(cands.find((c) => refKey(c) === addSel) ?? cands[0]);

  async function save(next: BackendRef[]) {
    try {
      // 链里的项是响应式代理，先取快照再提交
      await updateConfig({ chains: { [task]: normalize(task, $state.snapshot(next) as BackendRef[]) } });
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  async function patch(p: { fallback?: boolean; timeoutSecs?: number }) {
    try {
      await updateConfig({ chains: p });
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  const STATE_NOTE = { stale: '已失效', nokey: '没有 Key' } as const;
  const STATE_TIP = {
    stale: '服务商已移除、模型已取消勾选或不是视觉模型，执行时会跳过',
    nokey: '还没填 Key，执行到这里会报错并换下一个',
  } as const;
</script>

<div class="head">
  <h3 class="sec">备用链 · 失败时按顺序自动切换</h3>
  <Switch label="自动切换备用" checked={chains.fallback} onchange={(fallback) => patch({ fallback })} />
</div>

<div class="card" class:off={!chains.fallback}>
  <div class="bar">
    <Seg label="备用链" options={TASKS} value={task} onchange={(v) => ((task = v), (addSel = ''))} />
    <span class="grow"></span>
    <span class="lbl">超时</span>
    <Select
      aria-label="超时"
      width="96px"
      options={TIMEOUTS}
      value={String(chains.timeoutSecs)}
      onchange={(e) => patch({ timeoutSecs: Number(e.currentTarget.value) })}
    />
  </div>

  {#each chain as r, i (refKey(r))}
    {@const st = refState(r, task, providers, keyStatus, NO_KEY)}
    <div class="crow" class:dim={!chains.fallback && i > 0} animate:flip={{ duration: 200 }} in:fade={{ duration: 120 }}>
      <span class="num" class:first={i === 0}>{i + 1}</span>
      <span class="name" class:stale={st === 'stale'}>{refLabel(r, providers)}</span>
      {#if st !== 'ok'}<span class="warn" title={STATE_TIP[st]}>{STATE_NOTE[st]}</span>{/if}
      <span class="chip">{refTag(r, task)}</span>
      <IconButton icon="chevronUp" label="上移" size={28} disabled={i === 0 || r.type === 'degrade'} onclick={() => save(move(chain, i, i - 1))} />
      <IconButton
        icon="chevronDown"
        label="下移"
        size={28}
        disabled={i === chain.length - 1 || chain[i + 1]?.type === 'degrade' || r.type === 'degrade'}
        onclick={() => save(move(chain, i, i + 1))}
      />
      <IconButton icon="close" label="移除" size={28} onclick={() => save(chain.filter((_, j) => j !== i))} />
    </div>
  {:else}
    <div class="none">
      {#if task === 'translate'}还没有翻译后端。先在上面的「模型服务」里勾选模型或填好机器翻译的 Key，再加进来。
      {:else if task === 'recognize'}还没有识别后端，加一个本地 OCR 或视觉模型。
      {:else}还没有一步到位后端。需要勾选带「视觉」标签的模型。{/if}
    </div>
  {/each}

  <div class="add">
    {#if cands.length}
      <Select
        aria-label="添加备用"
        width="100%"
        options={cands.map((c) => ({ value: refKey(c), label: `${refLabel(c, providers)}（${refTag(c, task)}）` }))}
        value={pick ? refKey(pick) : ''}
        onchange={(e) => (addSel = e.currentTarget.value)}
      />
      <Button icon="plus" onclick={() => pick && save([...chain, pick])}>加入链尾</Button>
    {:else}
      <span class="t-note">没有可添加的项{task === 'translate' ? '' : '（识别和一步到位只能用带「视觉」标签的模型）'}</span>
    {/if}
  </div>
</div>
<p class="t-note tip">第 1 个是首选。请求失败、超时或返回空结果时，自动换下一个，结果卡片会标出实际用的是哪个。关掉自动切换时只用第 1 个。</p>

<style>
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 8px;
  }
  .sec {
    margin: 0;
    flex: 1;
    font-size: 14px;
    line-height: 20px;
    font-weight: 600;
  }
  .card {
    background: var(--card);
    border-radius: var(--r-panel);
    box-shadow: var(--card-ring);
    transition: opacity var(--d-hover) var(--ease-hover);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px 10px 14px;
    border-bottom: 1px solid var(--line);
  }
  .grow { flex: 1; }
  .lbl { font-size: 12px; color: var(--text2); }
  .crow {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px 6px 14px;
    min-height: 32px;
    background: var(--card);
    transition:
      background-color var(--d-hover) var(--ease-hover),
      opacity var(--d-hover) var(--ease-hover);
  }
  .crow + .crow { border-top: 1px solid var(--line); }
  .crow:hover { background: var(--hover); }
  .crow.dim { opacity: .45; }
  .num {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--sunken);
    font-size: 11px;
    font-weight: 600;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text2);
    flex: none;
    transition: background-color var(--d-hover) var(--ease-hover), color var(--d-hover) var(--ease-hover);
  }
  .num.first {
    background: var(--accent);
    color: var(--on-accent);
  }
  .name {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name.stale {
    color: var(--text3);
    text-decoration: line-through;
  }
  .warn {
    font-size: 11.5px;
    color: var(--danger);
    flex: none;
    cursor: help;
  }
  .chip {
    font-size: 12px;
    line-height: 22px;
    padding: 0 8px;
    border-radius: var(--r-tag);
    background: var(--chip);
    color: var(--chip-text);
    flex: none;
  }
  .none {
    padding: 18px 16px;
    font-size: 13px;
    line-height: 20px;
    color: var(--text3);
  }
  .add {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 12px 14px;
    border-top: 1px solid var(--line);
  }
  .add :global(.sel) { flex: 1; min-width: 0; }
  .tip { margin: -6px 0 0; color: var(--text3); }
</style>
