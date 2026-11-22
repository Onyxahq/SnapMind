<script lang="ts">
  // 设置页「模型服务」里的机器翻译服务：填 Key（存系统凭据库）、微软的区域、测试连接。
  import { fade } from 'svelte/transition';
  import { mtTest, providerSetKey } from '$lib/ipc';
  import { config, updateConfig } from '$lib/config.svelte';
  import { mtAccount, type MtService } from '$lib/providers';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Field from '$lib/ui/Field.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Icon from '$lib/ui/Icon.svelte';
  import { toast } from '$lib/ui/toast.svelte';

  let { svc, hasKey, onkey }: { svc: MtService; hasKey: boolean; onkey: (has: boolean) => void } = $props();

  let draft = $state('');
  let st = $state<{ kind: 'busy' | 'ok' | 'err'; text: string } | null>(null);
  let armClear = $state(false);

  async function saveKey() {
    if (!draft.trim()) return;
    try {
      await providerSetKey(mtAccount(svc.kind), draft);
      draft = '';
      onkey(true);
      await test();
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  async function clearKey() {
    if (!armClear) {
      armClear = true;
      setTimeout(() => (armClear = false), 3000);
      return;
    }
    armClear = false;
    try {
      await providerSetKey(mtAccount(svc.kind), '');
      onkey(false);
      st = null;
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  async function test() {
    st = { kind: 'busy', text: '正在连接…' };
    try {
      const out = await mtTest(svc.kind);
      st = { kind: 'ok', text: `连接成功 · Hello, world! → ${out}` };
    } catch (e) {
      st = { kind: 'err', text: String(e) };
    }
  }

  async function saveRegion(v: string) {
    try {
      await updateConfig({ mt: { microsoftRegion: v.trim() } });
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }
</script>

<header>
  <span class="logo big" style:background={svc.color}>{svc.mono}</span>
  <div class="ht">
    <b>{svc.name}</b>
    <span>{svc.desc}</span>
  </div>
  {#if hasKey}
    <Button variant={armClear ? 'danger' : 'ghost'} onclick={clearKey}>{armClear ? '确认删除 Key' : '删除 Key'}</Button>
  {/if}
</header>

<Card>
  <Row title="API Key" desc={svc.keyHint}>
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
        placeholder={hasKey ? '已保存 · 输入新的 Key 可替换' : '粘贴你的 API Key，回车保存'}
        bind:value={draft}
        onblur={() => void saveKey()}
      />
      {#if hasKey}<span class="saved" title="已保存在系统凭据库"><Icon name="lock" /></span>{/if}
    </form>
  </Row>
  {#if svc.kind === 'microsoft'}
    <Row title="区域" desc="Azure 里翻译资源的「位置/区域」，如 eastasia；全局资源留空">
      <Field
        mono
        width="300px"
        aria-label="区域"
        placeholder="global"
        spellcheck="false"
        value={config.value.mt.microsoftRegion}
        onchange={(e) => void saveRegion(e.currentTarget.value)}
      />
    </Row>
  {/if}
  <Row title="连接测试">
    {#if st}
      <span class="st {st.kind}" transition:fade={{ duration: 120 }}>{st.text}</span>
    {/if}
    <Button disabled={!hasKey || st?.kind === 'busy'} onclick={test}>测试连接</Button>
  </Row>
</Card>
<p class="t-note">机器翻译不需要选模型。加到「备用链」里就能用来翻译；只有填了 Key 的服务才会出现在可选列表里。</p>

<style>
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
  .logo.big {
    width: 40px;
    height: 40px;
    border-radius: 12px;
    font-size: 14px;
    font-weight: 700;
    color: #fff;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
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
  p { margin: 0; color: var(--text3); }
</style>
