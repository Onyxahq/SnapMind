<script lang="ts">
  // 设置 → 配置文件（需求第 10 节、M10.4）：位置；导出（选内容，API Key 可用密码加密导出）；
  // 导入（拖入或点击选择，预览后可只导入一部分，导入前自动备份）；自动备份与恢复；恢复默认。
  import { onMount } from 'svelte';
  import { slide } from '$lib/motion';
  import {
    clipboardWriteText,
    configBackups,
    configExport,
    configImport,
    configImportPick,
    configImportRead,
    configPath,
    configReset,
    configRestore,
    inTauri,
    openFolder,
    type BackupInfo,
    type BackupReason,
    type ConfigSection,
    type ImportPreview,
  } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Field from '$lib/ui/Field.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import Head from './Head.svelte';

  const SECTIONS: { id: ConfigSection; label: string; desc: string }[] = [
    { id: 'general', label: '常规设置', desc: '外观、截图与保存、翻译偏好、录屏、历史保留方式' },
    { id: 'keys', label: '快捷键与工具栏', desc: '全局快捷键、两条工具条的布局' },
    { id: 'prompts', label: '提示词', desc: '识别、翻译、一步到位、Markdown 四套' },
    { id: 'providers', label: '模型服务与备用链', desc: '服务商、模型、机器翻译设置、三条备用链' },
  ];
  const REASON: Record<BackupReason, string> = {
    import: '导入配置前',
    reset: '恢复默认前',
    restore: '恢复备份前',
    change: '修改重要设置前',
    weekly: '每周自动备份',
  };

  let path = $state('');
  let backups = $state<BackupInfo[]>([]);
  const folder = $derived(path.replace(/[\\/][^\\/]*$/, ''));

  async function refreshBackups() {
    try {
      backups = await configBackups();
    } catch {
      backups = [];
    }
  }

  // ---- 导出 ----
  let exp = $state<Record<ConfigSection, boolean>>({ general: true, keys: true, prompts: true, providers: true });
  let withKeys = $state(false);
  let pw = $state('');
  let pw2 = $state('');
  let exporting = $state(false);
  const picked = $derived(SECTIONS.filter((s) => exp[s.id]).map((s) => s.id));
  const keysOk = $derived(!withKeys || (pw.length >= 6 && pw === pw2));

  async function doExport() {
    if (!picked.length || !keysOk) return;
    exporting = true;
    try {
      const p = await configExport(picked, withKeys && exp.providers ? pw : undefined);
      if (p) toast(`已导出到 ${p}`);
    } catch (e) {
      toast(String(e), { kind: 'error' });
    } finally {
      exporting = false;
    }
  }

  // ---- 导入 ----
  let preview = $state<ImportPreview | null>(null);
  let imp = $state<Record<ConfigSection, boolean>>({ general: true, keys: true, prompts: true, providers: true });
  let ipw = $state('');
  let importing = $state(false);
  let dropping = $state(false);

  function show(p: ImportPreview) {
    preview = p;
    imp = { general: p.sections.includes('general'), keys: p.sections.includes('keys'), prompts: p.sections.includes('prompts'), providers: p.sections.includes('providers') };
    ipw = '';
  }

  async function pick() {
    try {
      const p = await configImportPick();
      if (p) show(p);
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  async function readDropped(p: string) {
    try {
      show(await configImportRead(p));
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  async function doImport() {
    if (!preview) return;
    const sections = SECTIONS.filter((s) => imp[s.id] && preview!.sections.includes(s.id)).map((s) => s.id);
    if (!sections.length) return;
    importing = true;
    try {
      const r = await configImport(preview.path, sections, preview.hasKeys && imp.providers ? ipw : undefined);
      toast(`已导入${r.keys ? `，并写回 ${r.keys} 个 API Key` : ''}；导入前的配置已备份`);
      preview = null;
      refreshBackups();
    } catch (e) {
      toast(String(e), { kind: 'error' });
    } finally {
      importing = false;
    }
  }

  // ---- 恢复 / 恢复默认：要点两次 ----
  let armed = $state<string | null>(null);
  async function restore(b: BackupInfo) {
    if (armed !== b.name) {
      armed = b.name;
      return;
    }
    armed = null;
    try {
      await configRestore(b.name);
      toast(`已恢复到 ${b.time} 的配置`);
      refreshBackups();
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }
  async function reset() {
    if (armed !== '__reset') {
      armed = '__reset';
      return;
    }
    armed = null;
    try {
      await configReset();
      toast('已恢复默认设置（模型服务保留）；之前的配置已备份');
      refreshBackups();
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  onMount(() => {
    configPath().then((p) => (path = p));
    refreshBackups();
    if (!inTauri()) return;
    // 把 .json 拖进设置窗口
    let off: (() => void) | undefined;
    import('@tauri-apps/api/webview').then(async ({ getCurrentWebview }) => {
      off = await getCurrentWebview().onDragDropEvent((e) => {
        const t = e.payload.type;
        if (t === 'enter' || t === 'over') dropping = true;
        else if (t === 'leave') dropping = false;
        else if (t === 'drop') {
          dropping = false;
          const p = e.payload.paths.find((x) => x.toLowerCase().endsWith('.json'));
          if (p) readDropped(p);
          else toast('请拖入 .json 配置文件', { kind: 'error' });
        }
      });
    });
    return () => off?.();
  });

  async function copyPath() {
    await clipboardWriteText(path);
    toast('已复制配置文件路径');
  }
</script>

<Head title="配置文件" desc="所有设置都存在一个 JSON 文件里，可以导出备份、在别的电脑导入。API Key 不在里面，存在系统凭据管理器中。" />

<Card>
  <Row title="配置文件位置" desc={path || '…'}>
    <div class="btns">
      <Button variant="ghost" icon="folder" onclick={() => folder && openFolder(folder)}>打开文件夹</Button>
      <Button variant="ghost" icon="copy" onclick={copyPath}>复制路径</Button>
    </div>
  </Row>
</Card>

<Card title="导出">
  <div class="checks">
    {#each SECTIONS as s (s.id)}
      <label class="ck" title={s.desc}><input type="checkbox" bind:checked={exp[s.id]} />{s.label}</label>
    {/each}
  </div>
  <Row title="包含 API Key" desc={exp.providers ? '默认不导出；打开后用密码加密，导入时要输入同一个密码' : '要先勾选「模型服务与备用链」'}>
    <Switch label="包含 API Key" checked={withKeys && exp.providers} disabled={!exp.providers} onchange={(v) => (withKeys = v)} />
  </Row>
  {#if withKeys && exp.providers}
    <div class="pw" transition:slide={{ duration: 160 }}>
      <Field type="password" width="200px" aria-label="加密密码" placeholder="密码（至少 6 位）" value={pw} oninput={(e) => (pw = e.currentTarget.value)} />
      <Field type="password" width="200px" aria-label="再输一次密码" placeholder="再输一次" value={pw2} oninput={(e) => (pw2 = e.currentTarget.value)} />
      <span class="hint" class:bad={pw2.length > 0 && pw !== pw2}>
        {pw2.length > 0 && pw !== pw2 ? '两次输入不一样' : '忘了密码就解不开 Key，只能重新填'}
      </span>
    </div>
  {/if}
  <div class="foot">
    <span class="count">将导出 {picked.length} 项内容{withKeys && exp.providers ? '，含加密的 API Key' : ''}</span>
    <Button variant="primary" disabled={!picked.length || !keysOk || exporting} onclick={doExport}>{exporting ? '正在导出…' : '导出配置…'}</Button>
  </div>
</Card>

<Card title="导入">
  {#if preview}
    <div class="preview" transition:slide={{ duration: 160 }}>
      <div class="pv-head">
        <Icon name="check" size={16} />
        <span class="pv-name" title={preview.path}>{preview.path.split(/[\\/]/).pop()}</span>
        <span class="pv-meta">SnapMind {preview.appVersion} · {preview.exportedAt}</span>
      </div>
      <div class="checks">
        {#each SECTIONS as s (s.id)}
          {@const has = preview.sections.includes(s.id)}
          <label class="ck" class:off={!has} title={has ? s.desc : '文件里没有这一项'}>
            <input type="checkbox" bind:checked={imp[s.id]} disabled={!has} />{s.label}
          </label>
        {/each}
      </div>
      {#if preview.hasKeys && imp.providers}
        <div class="pw">
          <Field type="password" width="200px" aria-label="解密密码" placeholder="导出时设的密码" value={ipw} oninput={(e) => (ipw = e.currentTarget.value)} />
          <span class="hint">文件里有加密的 API Key；不填密码就只导入设置、不导入 Key</span>
        </div>
      {/if}
      <div class="foot">
        <span class="count">导入前会自动备份当前配置，选中的几类会整块替换</span>
        <Button variant="ghost" onclick={() => (preview = null)}>取消</Button>
        <Button variant="primary" disabled={importing || !SECTIONS.some((s) => imp[s.id] && preview!.sections.includes(s.id))} onclick={doImport}>
          {importing ? '正在导入…' : '导入'}
        </Button>
      </div>
    </div>
  {:else}
    <button type="button" class="drop" class:hot={dropping} onclick={pick}>
      <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 15V4M7.5 8.5L12 4l4.5 4.5M5 20h14" /></svg>
      把 .json 配置文件拖到这里，或点击选择
      <small>导入前会自动备份当前配置；可以只导入其中一部分</small>
    </button>
  {/if}
</Card>

<div class="bk-head">
  <h3>自动备份</h3>
  <Switch label="自动备份" checked={config.value.backup.auto} onchange={(auto) => updateConfig({ backup: { auto } }).catch((e) => toast(String(e), { kind: 'error' }))} />
</div>
<p class="bk-note">修改重要设置前（最多每小时一份）和每周各备份一次；导入、恢复、恢复默认前总会备份。保留最近 20 份。</p>
<div class="card">
  {#if backups.length}
    {#each backups as b (b.name)}
      <div class="brow">
        <div class="bt"><b>{b.time}</b><span>{REASON[b.reason]}</span></div>
        <Button variant={armed === b.name ? 'danger' : 'ghost'} onclick={() => restore(b)} onblur={() => armed === b.name && (armed = null)}>
          {armed === b.name ? '再点一次恢复' : '恢复'}
        </Button>
      </div>
    {/each}
  {:else}
    <div class="empty">还没有备份</div>
  {/if}
</div>

<Card>
  <Row title="恢复默认设置" desc="快捷键、提示词、工具栏、外观等全部还原；模型服务、备用链和历史记录保留（之前的配置会先备份）">
    <Button variant="danger" onclick={reset} onblur={() => armed === '__reset' && (armed = null)}>{armed === '__reset' ? '再点一次恢复默认' : '恢复默认'}</Button>
  </Row>
</Card>

<style>
  .btns { display: flex; gap: 6px; }
  .checks { display: flex; flex-wrap: wrap; gap: 10px 22px; padding: 14px 16px; }
  .ck { display: inline-flex; align-items: center; gap: 8px; font-size: 13px; cursor: pointer; }
  .ck.off { color: var(--text3); cursor: default; }
  .ck input { width: 16px; height: 16px; margin: 0; accent-color: var(--accent); }
  .pw { display: flex; align-items: center; flex-wrap: wrap; gap: 10px; padding: 0 16px 12px; }
  .hint { font-size: 12px; color: var(--text3); }
  .hint.bad { color: var(--danger); }
  .foot { display: flex; align-items: center; gap: 8px; padding: 12px 16px; border-top: 1px solid var(--line); }
  .count { flex: 1; font-size: 12px; color: var(--text3); }
  .drop {
    margin: 14px 16px; width: calc(100% - 32px); box-sizing: border-box;
    display: flex; flex-direction: column; align-items: center; gap: 6px; padding: 22px;
    border: 1.5px dashed var(--line2); border-radius: 10px; background: transparent;
    font: inherit; font-size: 13px; color: var(--text2); cursor: pointer;
    transition: border-color var(--d-hover) var(--ease-hover), background var(--d-hover) var(--ease-hover);
  }
  .drop:hover, .drop.hot { border-color: var(--accent); background: var(--accent-soft); }
  .drop svg { width: 22px; height: 22px; fill: none; stroke: var(--accent); stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  .drop small { font-size: 12px; color: var(--text3); }
  .preview { display: flex; flex-direction: column; }
  .pv-head { display: flex; align-items: center; gap: 8px; padding: 14px 16px 0; color: var(--accent); }
  .pv-name { font-size: 13px; font-weight: 600; color: var(--text); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .pv-meta { margin-left: auto; font-size: 12px; color: var(--text3); white-space: nowrap; }
  .bk-head { display: flex; align-items: center; gap: 10px; margin-top: 8px; }
  .bk-head h3 { flex: 1; margin: 0; font-size: 14px; line-height: 20px; font-weight: 600; }
  .bk-note { margin: -8px 0 0; font-size: 12px; color: var(--text3); }
  .card { border-radius: var(--r-panel); background: var(--card); box-shadow: var(--card-ring); }
  .brow { display: flex; align-items: center; gap: 12px; padding: 10px 16px; }
  .brow + .brow { border-top: 1px solid var(--line); }
  .bt { flex: 1; display: flex; flex-direction: column; gap: 2px; }
  .bt b { font-size: 13px; font-weight: 500; font-variant-numeric: tabular-nums; }
  .bt span { font-size: 12px; color: var(--text3); }
  .empty { padding: 24px; text-align: center; font-size: 13px; color: var(--text3); }
</style>
