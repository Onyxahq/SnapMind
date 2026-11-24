<script lang="ts">
  // 设置 → 通用：显示语言、开机自动启动、网络代理；macOS 上还有系统权限。
  import { onMount } from 'svelte';
  import { appInfo, appRestart, autostartGet, autostartSet, permissionRequestAccessibility, permissionRequestScreen, permissionsGet, type Permissions } from '$lib/ipc';
  import { config, systemTag, updateConfig } from '$lib/config.svelte';
  import { LANGUAGES, matchLocale, t, type LanguageSetting } from '$lib/i18n.svelte';
  import { toast } from '$lib/ui/toast.svelte';
  import Card from '$lib/ui/Card.svelte';
  import Row from '$lib/ui/Row.svelte';
  import Select from '$lib/ui/Select.svelte';
  import Switch from '$lib/ui/Switch.svelte';
  import Field from '$lib/ui/Field.svelte';
  import Button from '$lib/ui/Button.svelte';
  import Head from './Head.svelte';

  let autostart = $state(false);
  let mac = $state(false);
  let perms = $state<Permissions | null>(null);
  /** 本次打开设置后点过「去授权」：系统要重启应用才让新权限生效，提示并给出重启按钮 */
  let asked = $state(false);

  const refreshPerms = () => permissionsGet().then((p) => (perms = p));

  onMount(() => {
    autostartGet().then((a) => (autostart = a));
    appInfo().then((i) => {
      mac = i.platform === 'macos';
      if (mac) refreshPerms();
    });
    // 用户去系统设置里勾选后切回来，刷新一下状态
    const onFocus = () => mac && refreshPerms();
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  });

  async function requestScreen() {
    asked = true;
    await permissionRequestScreen();
  }

  /** 「跟随系统」那一项顺带写出现在跟随到了哪种语言 */
  const options = $derived.by(() => {
    const now = LANGUAGES.find((l) => l.code === matchLocale(systemTag()))!.name;
    return [
      { value: 'system' as LanguageSetting, label: `${t('跟随系统')} · ${now}` },
      ...LANGUAGES.map((l) => ({ value: l.code as LanguageSetting, label: l.name })),
    ];
  });

  async function setLanguage(language: LanguageSetting) {
    try {
      await updateConfig({ general: { language } });
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  async function saveProxy(patch: Partial<typeof config.value.proxy>) {
    try {
      await updateConfig({ proxy: patch });
    } catch (e) {
      toast(String(e), { kind: 'error' });
    }
  }

  async function setAutostart(on: boolean) {
    autostart = await autostartSet(on);
  }
</script>

<Head title={t('通用')} desc={t('界面语言和启动方式。')} />

<Card>
  <Row title={t('显示语言')} desc={t('默认跟随系统语言，系统语言不在列表里时使用 English。切换后立即生效。')}>
    <Select
      aria-label={t('显示语言')}
      width="220px"
      {options}
      value={config.value.general.language}
      onchange={(e) => setLanguage(e.currentTarget.value as LanguageSetting)}
    />
  </Row>
  <Row title={t('开机自动启动')} desc={t('开机后在托盘里待命，按快捷键就能截图；托盘菜单里也有同一个开关')}>
    <Switch label={t('开机自动启动')} checked={autostart} onchange={setAutostart} />
  </Row>
</Card>

{#if mac && perms}
  <Card title={t('系统权限')}>
    <Row
      title={t('屏幕录制')}
      desc={perms.screen
        ? t('已允许。截图、贴图、识别文字都要用到它。')
        : t('截图必需。点「去授权」后在系统设置里打开 SnapMind 的开关，再重启 SnapMind 生效。')}
    >
      {#if perms.screen}
        <span class="ok">{t('已允许')}</span>
      {:else}
        {#if asked}<Button onclick={appRestart}>{t('重启 SnapMind')}</Button>{/if}
        <Button variant="primary" onclick={requestScreen}>{t('去授权')}</Button>
      {/if}
    </Row>
    <Row
      title={t('辅助功能')}
      desc={perms.accessibility
        ? t('已允许。截图时框出窗口里的按钮和输入框、长截图自动滚动、划词翻译都要用到它。')
        : t('选用。截图时框出窗口里的按钮和输入框、长截图自动滚动、划词翻译要用到；不授权时截图只能框出整个窗口。点「去授权」后在系统设置里打开 SnapMind 的开关，立即生效。')}
    >
      {#if perms.accessibility}
        <span class="ok">{t('已允许')}</span>
      {:else}
        <Button variant="primary" onclick={permissionRequestAccessibility}>{t('去授权')}</Button>
      {/if}
    </Row>
  </Card>
{/if}

<Card title={t('网络代理')}>
  <Row title={t('通过代理联网')} desc={t('开启后，SnapMind 访问大模型、翻译服务、模型下载和检查更新都走下面的代理；本机服务（如 Ollama）不受影响。关闭时沿用系统的代理设置。')}>
    <Switch label={t('通过代理联网')} checked={config.value.proxy.enabled} onchange={(enabled) => saveProxy({ enabled })} />
  </Row>
  {#if config.value.proxy.enabled}
    <Row title={t('代理地址')} desc={t('例如 127.0.0.1:7890；也可以带上协议：http://、https://、socks5://')}>
      <Field
        width="240px"
        aria-label={t('代理地址')}
        placeholder="127.0.0.1:7890"
        value={config.value.proxy.address}
        onchange={(e) => saveProxy({ address: e.currentTarget.value.trim() })}
      />
    </Row>
    <Row title={t('代理不通时直接联网')} desc={t('代理软件没开或连不上时，这次改为不经代理直接访问；默认关闭，此时会提示连接失败。直接联网会暴露你的真实网络地址。')}>
      <Switch label={t('代理不通时直接联网')} checked={config.value.proxy.fallbackDirect} onchange={(fallbackDirect) => saveProxy({ fallbackDirect })} />
    </Row>
  {/if}
</Card>

<style>
  .ok {
    font-size: 13px;
    color: var(--accent);
    white-space: nowrap;
  }
</style>
