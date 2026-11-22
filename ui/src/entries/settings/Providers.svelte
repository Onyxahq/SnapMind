<script lang="ts">
  // 设置页「模型服务」（需求 1.4）：左边服务商列表，右边选中服务商的地址、Key 和模型。
  // Key 只进系统凭据库，前端只知道「有没有」，输入框永远不回显。
  import { onMount } from 'svelte';
  import { fade } from 'svelte/transition';
  import {
    providerCatalog,
    providerFetchModels,
    providerKeyStatus,
    providerSetKey,
    type ModelConfig,
    type ModelTag,
    type ProviderConfig,
    type ProviderKind,
    type ProviderMeta,
  } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import { FORMAT_LABEL, TAGS, canAdd, mergeModels, newProviderId, toggleTag, visibleModels } from '$lib/providers';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Field from '$lib/ui/Field.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  let catalog = $state<ProviderMeta[]>([]);
  let keys = $state<Record<string, boolean>>({});
  /** 选中的服务商 id，或者 `add:<kind>`（还没添加的内置服务商） */
  let sel = $state('');
  let keyDraft = $state('');
  let filter = $state('');
  let manual = $state('');
  let armRemove = $state(false);
  type Status = { kind: 'busy' | 'ok' | 'err'; text: string };
  let status = $state<Record<string, Status>>({});

  const providers = $derived(config.value.providers);
  const metaOf = (kind: ProviderKind) => catalog.find((m) => m.kind === kind);
  const cur = $derived(providers.find((p) => p.id === sel));
  const curMeta = $derived(cur ? metaOf(cur.kind) : metaOf(sel.slice(4) as ProviderKind));
  const addable = $derived(catalog.filter((m) => canAdd(m.kind, providers)));
  const shown = $derived(cur ? visibleModels(cur.models, filter) : []);
  const st = $derived(cur ? status[cur.id] : undefined);

  onMount(async () => {
    catalog = await providerCatalog();
    keys = await providerKeyStatus(providers.map((p) => p.id));
    sel = providers[0]?.id ?? `add:${catalog[0]?.kind ?? 'deepseek'}`;
  });

  function pick(id: string) {
    if (id === sel) return;
    sel = id;
    keyDraft = '';
    filter = '';
    manual = '';
    armRemove = false;
  }

  async function commit(list: ProviderConfig[]) {
    try {
      await updateConfig({ providers: list });
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  function patchCur(f: (p: ProviderConfig) => ProviderConfig) {
    const id = cur?.id;
    if (!id) return Promise.resolve();
    return commit(providers.map((p) => (p.id === id ? f(structuredClone($state.snapshot(p)) as ProviderConfig) : p)));
  }

  async function add(m: ProviderMeta) {
    const id = newProviderId(m.kind, providers);
    const n = providers.filter((p) => p.kind === m.kind).length;
    const name = m.kind === 'custom' && n > 0 ? `${m.name} ${n + 1}` : m.name;
    await commit([...providers, { id, kind: m.kind, name, baseUrl: m.defaultUrl, models: [] }]);
    keys[id] = false;
    pick(id);
    // 不需要 Key 的服务（Ollama）加上就去取一次模型
    if (!m.needsKey) void fetchModels();
  }

  async function remove() {
    if (!cur) return;
    if (!armRemove) {
      armRemove = true;
      setTimeout(() => (armRemove = false), 3000);
      return;
    }
    const { id, kind } = cur;
    try {
      await providerSetKey(id, '');
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
    const rest = providers.filter((p) => p.id !== id);
    await commit(rest);
    delete keys[id];
    delete status[id];
    sel = '';
    pick(rest[0]?.id ?? `add:${kind}`);
  }

  async function saveKey() {
    if (!cur || !keyDraft.trim()) return;
    try {
      await providerSetKey(cur.id, keyDraft);
      keys[cur.id] = true;
      keyDraft = '';
      // 填好 Key 的下一步一定是取模型：直接取，顺便验证 Key
      await fetchModels();
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  /** merge=false 只测试连接，不改模型列表。 */
  async function fetchModels(merge = true) {
    const p = cur;
    if (!p) return;
    const id = p.id;
    status[id] = { kind: 'busy', text: merge ? '正在获取模型列表…' : '正在连接…' };
    try {
      const got = await providerFetchModels({ id, kind: p.kind, baseUrl: p.baseUrl });
      if (merge) {
        const latest = config.value.providers.find((x) => x.id === id);
        if (latest) {
          const models = mergeModels($state.snapshot(latest.models) as ModelConfig[], got);
          await commit(config.value.providers.map((x) => (x.id === id ? { ...x, models } : x)));
        }
      }
      status[id] = { kind: 'ok', text: `连接成功 · 服务商提供 ${got.length} 个模型` };
    } catch (e) {
      status[id] = { kind: 'err', text: String(e) };
    }
  }

  function setModel(mid: string, f: (m: ModelConfig) => ModelConfig) {
    return patchCur((p) => ({ ...p, models: p.models.map((m) => (m.id === mid ? f(m) : m)) }));
  }

  async function addManual() {
    const id = manual.trim();
    if (!cur || !id) return;
    if (cur.models.some((m) => m.id === id)) {
      await setModel(id, (m) => ({ ...m, enabled: true }));
    } else {
      const tags: ModelTag[] = cur.kind === 'ollama' ? ['text', 'local'] : ['text'];
      await patchCur((p) => ({ ...p, models: [{ id, tags, enabled: true }, ...p.models] }));
    }
    manual = '';
  }

  const enabledCount = (p: ProviderConfig) => p.models.filter((m) => m.enabled).length;
</script>

<div class="wrap">
  <nav class="list" aria-label="服务商">
    {#if providers.length}
      <span class="cap">已添加</span>
      {#each providers as p (p.id)}
        {@const m = metaOf(p.kind)}
        <button type="button" class="prov" class:on={sel === p.id} onclick={() => pick(p.id)}>
          <span class="logo" style:background={m?.color}>{m?.mono}</span>
          <span class="pt">
            <b>{p.name}</b>
            <small>
              {#if m?.needsKey && !keys[p.id]}还没填 Key{:else}{enabledCount(p)} 个模型已显示{/if}
            </small>
          </span>
        </button>
      {/each}
    {/if}
    {#if addable.length}
      <span class="cap">可添加</span>
      {#each addable as m (m.kind)}
        <button type="button" class="prov more" class:on={sel === `add:${m.kind}`} onclick={() => pick(`add:${m.kind}`)}>
          <span class="logo dim" style:background={m.color}>{m.mono}</span>
          <span class="pt"><span class="nm">{m.name}</span></span>
          <Icon name="plus" />
        </button>
      {/each}
    {/if}
  </nav>

  {#key sel}
    <div class="detail" in:fade={{ duration: 120 }}>
      {#if curMeta}
        <header>
          <span class="logo big" style:background={curMeta.color}>{curMeta.mono}</span>
          <div class="ht">
            <b>{cur?.name ?? curMeta.name}</b>
            <span>{curMeta.desc}</span>
          </div>
          {#if cur}
            <Button variant={armRemove ? 'danger' : 'ghost'} onclick={remove}>{armRemove ? '确认移除' : '移除'}</Button>
          {/if}
        </header>

        {#if !cur}
          <Card>
            <div class="empty">
              <span>添加后填写 API Key，就能获取并选择这个服务商的模型。</span>
              <Button variant="primary" icon="plus" onclick={() => add(curMeta)}>添加 {curMeta.name}</Button>
            </div>
          </Card>
        {:else}
          <Card>
            <Row title="接口格式">
              <span class="chip">{FORMAT_LABEL[curMeta.format]}</span>
            </Row>
            {#if cur.kind === 'custom'}
              <Row title="名称">
                <Field
                  width="300px"
                  aria-label="名称"
                  value={cur.name}
                  onchange={(e) => {
                    const name = e.currentTarget.value.trim() || curMeta.name;
                    void patchCur((p) => ({ ...p, name }));
                  }}
                />
              </Row>
            {/if}
            <Row title="Base URL" desc={cur.kind === 'custom' ? '填到 /v1 为止' : undefined}>
              <Field
                mono
                width="300px"
                aria-label="Base URL"
                value={cur.baseUrl}
                spellcheck="false"
                onchange={(e) => {
                  const baseUrl = e.currentTarget.value.trim() || curMeta.defaultUrl;
                  void patchCur((p) => ({ ...p, baseUrl }));
                }}
              />
            </Row>
            <Row
              title="API Key"
              desc={curMeta.needsKey ? '只保存在本机的系统凭据库里' : '本地服务不需要 Key（填了也会带上）'}
            >
              <form
                class="key"
                onsubmit={(e) => {
                  e.preventDefault();
                  void saveKey();
                }}
              >
                <Field
                  type="password"
                  width="300px"
                  aria-label="API Key"
                  autocomplete="off"
                  placeholder={keys[cur.id] ? '已保存 · 输入新的 Key 可替换' : curMeta.needsKey ? '粘贴你的 API Key，回车保存' : '不需要'}
                  bind:value={keyDraft}
                  onblur={() => void saveKey()}
                />
                {#if keys[cur.id]}<span class="saved" title="已保存在系统凭据库"><Icon name="lock" /></span>{/if}
              </form>
            </Row>
            <Row title="连接测试">
              {#if st}
                <span class="st {st.kind}" transition:fade={{ duration: 120 }}>{st.text}</span>
              {/if}
              <Button disabled={st?.kind === 'busy'} onclick={() => fetchModels(false)}>测试连接</Button>
            </Row>
          </Card>

          <div class="mhead">
            <h3 class="sec">模型 · 勾选后显示</h3>
            {#if cur.models.length > 8}
              <Field width="160px" aria-label="筛选模型" placeholder="筛选" bind:value={filter} />
            {/if}
            <Button disabled={st?.kind === 'busy'} onclick={() => fetchModels(true)}>获取模型列表</Button>
          </div>

          <Card>
            <div class="models">
              {#each shown as m (m.id)}
                <div class="mrow" class:off={!m.enabled}>
                  <label class="mcheck">
                    <input
                      type="checkbox"
                      checked={m.enabled}
                      onchange={(e) => {
                        const enabled = e.currentTarget.checked;
                        void setModel(m.id, (x) => ({ ...x, enabled }));
                      }}
                    />
                    <span class="mid">{m.id}</span>
                  </label>
                  <span class="tags">
                    {#each TAGS as t (t.tag)}
                      <button
                        type="button"
                        class="tag"
                        class:on={m.tags.includes(t.tag)}
                        aria-pressed={m.tags.includes(t.tag)}
                        title={m.tags.includes(t.tag) ? `去掉「${t.label}」` : `标为「${t.label}」`}
                        onclick={() => setModel(m.id, (x) => ({ ...x, tags: toggleTag(x.tags, t.tag) }))}
                      >{t.label}</button>
                    {/each}
                  </span>
                </div>
              {:else}
                <div class="none">
                  {#if filter}没有匹配「{filter}」的模型{:else if curMeta.needsKey && !keys[cur.id]}填好 API Key 后会自动获取模型列表{:else}还没有模型，点「获取模型列表」或在下面手动添加{/if}
                </div>
              {/each}
            </div>
            <form
              class="manual"
              onsubmit={(e) => {
                e.preventDefault();
                void addManual();
              }}
            >
              <Field mono width="100%" aria-label="手动添加模型" placeholder="手动添加：输入模型 ID 后回车" spellcheck="false" bind:value={manual} />
              <Button type="submit" icon="plus" disabled={!manual.trim()}>添加</Button>
            </form>
          </Card>
        {/if}
      {/if}
    </div>
  {/key}
</div>

<style>
  .wrap {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }
  .list {
    width: 200px;
    flex: none;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: var(--card);
    border-radius: var(--r-panel);
    box-shadow: var(--card-ring);
  }
  .cap {
    font-size: 11px;
    color: var(--text3);
    padding: 8px 10px 2px;
    letter-spacing: .06em;
  }
  .prov {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 42px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    font: inherit;
    color: var(--text);
    cursor: pointer;
    text-align: left;
    transition:
      background-color var(--d-hover) var(--ease-hover),
      transform var(--d-press) var(--ease-press);
  }
  .prov:hover { background: var(--hover); }
  .prov:active { transform: scale(.98); }
  .prov.on { background: var(--accent-soft); }
  .prov:focus-visible { outline: none; box-shadow: 0 0 0 3px var(--accent-soft); }
  .prov.more { color: var(--text3); }
  .pt {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .pt b {
    font-size: 13px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pt small { font-size: 11px; color: var(--text3); }
  .nm { font-size: 13px; color: var(--text2); }
  .logo {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    font-weight: 700;
    color: #fff;
    flex: none;
  }
  .logo.dim { opacity: .55; }
  .logo.big {
    width: 40px;
    height: 40px;
    border-radius: 12px;
    font-size: 14px;
  }

  .detail {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .ht {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .ht b { font-size: 18px; line-height: 26px; font-weight: 600; }
  .ht span { font-size: 12px; color: var(--text2); }
  .empty {
    padding: 24px;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 12px;
    font-size: 13px;
    line-height: 20px;
    color: var(--text2);
  }
  .chip {
    font-size: 12px;
    line-height: 22px;
    padding: 0 8px;
    border-radius: var(--r-tag);
    background: var(--chip);
    color: var(--chip-text);
  }
  .key {
    position: relative;
    display: flex;
  }
  .saved {
    position: absolute;
    right: 9px;
    top: 9px;
    color: var(--text3);
    pointer-events: none;
    font-size: 16px;
  }
  .st {
    font-size: 12px;
    line-height: 18px;
    color: var(--text2);
    max-width: 320px;
    text-align: right;
  }
  .st.ok { color: var(--accent); }
  .st.err { color: var(--danger); }

  .mhead {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
  }
  .sec {
    margin: 0;
    flex: 1;
    font-size: 14px;
    line-height: 20px;
    font-weight: 600;
  }
  .models {
    max-height: 360px;
    overflow: auto;
    scrollbar-width: thin;
  }
  .mrow {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px 0 14px;
    min-height: 40px;
    transition: background-color var(--d-hover) var(--ease-hover);
  }
  .mrow + .mrow { border-top: 1px solid var(--line); }
  .mrow:hover { background: var(--hover); }
  .mcheck {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    align-self: stretch;
  }
  .mcheck input {
    accent-color: var(--accent);
    width: 16px;
    height: 16px;
    margin: 0;
    cursor: pointer;
    flex: none;
  }
  .mid {
    font-family: var(--font-mono);
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: color var(--d-hover) var(--ease-hover);
  }
  .mrow.off .mid { color: var(--text2); }
  .tags {
    display: flex;
    gap: 4px;
    flex: none;
  }
  .tag {
    height: 22px;
    padding: 0 7px;
    border: 0;
    border-radius: var(--r-tag);
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
    background: transparent;
    color: var(--text3);
    opacity: 0;
    transition:
      opacity var(--d-hover) var(--ease-hover),
      background-color var(--d-hover) var(--ease-hover),
      transform var(--d-press) var(--ease-press);
  }
  /* 没打上的标签平时隐藏，悬停这一行才出现，点一下就打上 */
  .mrow:hover .tag,
  .tag:focus-visible { opacity: 1; }
  .tag:hover { background: var(--hover); }
  .tag:active { transform: scale(.94); }
  .tag.on {
    opacity: 1;
    background: var(--chip);
    color: var(--chip-text);
  }
  .tag:focus-visible { outline: none; box-shadow: 0 0 0 2px var(--accent-soft); }
  .none {
    padding: 20px 16px;
    font-size: 13px;
    color: var(--text3);
  }
  .manual {
    display: flex;
    gap: 8px;
    padding: 10px 10px 10px 14px;
    border-top: 1px solid var(--line);
  }
  .manual :global(.field) { flex: 1; }
</style>
