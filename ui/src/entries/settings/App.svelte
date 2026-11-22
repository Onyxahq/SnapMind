<script lang="ts">
  // 设置窗口（需求第 9 节、M10.1）：左边导航十页，右边当前页。上次停在哪一页下次还打开哪一页。
  import { fade } from 'svelte/transition';
  import ToastHost from '$lib/ui/ToastHost.svelte';
  import PageRecognize from './PageRecognize.svelte';
  import Providers from './Providers.svelte';
  import Prompts from './Prompts.svelte';
  import PageKeys from './PageKeys.svelte';
  import PageToolbar from './PageToolbar.svelte';
  import PageCapture from './PageCapture.svelte';
  import PageHistory from './PageHistory.svelte';
  import PageAppearance from './PageAppearance.svelte';
  import PageConfig from './PageConfig.svelte';
  import PageAbout from './PageAbout.svelte';
  import Head from './Head.svelte';

  const PAGES = [
    { id: 'ocr', label: '识别与翻译', d: 'M3 5h9M7.5 3v2M5 5c.5 3.5 3 6.5 6.5 8M10 5c-.8 3.8-3 6.8-6.5 8.5M13 21l4-10 4 10M14.5 17.5h5' },
    { id: 'prov', label: '模型服务', d: 'M12 3l1.8 4.7 4.7 1.8-4.7 1.8L12 16l-1.8-4.7-4.7-1.8 4.7-1.8zM18 15l.8 2.2 2.2.8-2.2.8L18 21l-.8-2.2-2.2-.8 2.2-.8z' },
    { id: 'prompt', label: '提示词', d: 'M4 5h16v11H8l-4 4zM8 9h8M8 12h5' },
    { id: 'keys', label: '快捷键', d: 'M4 6h16a1 1 0 011 1v10a1 1 0 01-1 1H4a1 1 0 01-1-1V7a1 1 0 011-1zM7 10h.01M11 10h.01M15 10h.01M8 14h8' },
    { id: 'bar', label: '工具栏', d: 'M4 8h16M4 16h16M8 5v6M15 13v6' },
    { id: 'shot', label: '截图与贴图', d: 'M6 3v13a2 2 0 002 2h13M3 6h13a2 2 0 012 2v13' },
    { id: 'hist', label: '历史记录', d: 'M3.5 12a8.5 8.5 0 102.5-6M3.5 4v4h4M12 8v4l3 2' },
    { id: 'look', label: '外观', d: 'M12 3a9 9 0 100 18 9 9 0 000-18zM12 3v18M12 8h6.5M12 12h9M12 16h6.5' },
    { id: 'conf', label: '配置文件', d: 'M14 3H7a2 2 0 00-2 2v14a2 2 0 002 2h10a2 2 0 002-2V8zM14 3v5h5M9 13h6M9 17h4' },
    { id: 'about', label: '关于', d: 'M12 3a9 9 0 110 18 9 9 0 010-18zM12 11v5M12 8h.01' },
  ] as const;
  type PageId = (typeof PAGES)[number]['id'];
  const KEY = 'snapmind.settings.page';

  function load(): PageId {
    try {
      const v = localStorage.getItem(KEY);
      if (v && PAGES.some((p) => p.id === v)) return v as PageId;
    } catch {
      /* 没有本地存储时从第一页开始 */
    }
    return 'ocr';
  }

  let page = $state<PageId>(load());
  let scroller = $state<HTMLElement>();

  function go(id: PageId) {
    if (id === page) return;
    page = id;
    scroller?.scrollTo({ top: 0 });
    try {
      localStorage.setItem(KEY, id);
    } catch {
      /* 记不住也没关系 */
    }
  }
</script>

<div class="shell">
  <nav aria-label="设置分类">
    <div class="brand">
      <span class="logo" aria-hidden="true">
        <svg viewBox="0 0 24 24"><path d="M4 8V5.5A1.5 1.5 0 015.5 4H8M16 4h2.5A1.5 1.5 0 0120 5.5V8M20 16v2.5a1.5 1.5 0 01-1.5 1.5H16M8 20H5.5A1.5 1.5 0 014 18.5V16M8 10h8M8 14h5" /></svg>
      </span>
      <span class="bt"><b>SnapMind</b><small>截图 · 识别 · 翻译</small></span>
    </div>
    {#each PAGES as p (p.id)}
      <button type="button" class="nav" class:on={page === p.id} aria-current={page === p.id ? 'page' : undefined} onclick={() => go(p.id)}>
        <svg viewBox="0 0 24 24" aria-hidden="true"><path d={p.d} /></svg>{p.label}
      </button>
    {/each}
  </nav>

  <main bind:this={scroller}>
    {#key page}
      <div class="page" in:fade={{ duration: 140 }}>
        {#if page === 'ocr'}
          <PageRecognize />
        {:else if page === 'prov'}
          <Head title="模型服务" desc="先添加服务商并填写 Key，再勾选要显示的模型。勾选的模型才会出现在下拉框和备用链里。" />
          <Providers />
        {:else if page === 'prompt'}
          <Head title="提示词" desc="只在用到大模型时生效。每种任务一套提示词，可以随时恢复默认。" />
          <Prompts />
        {:else if page === 'keys'}
          <PageKeys />
        {:else if page === 'bar'}
          <PageToolbar />
        {:else if page === 'shot'}
          <PageCapture />
        {:else if page === 'hist'}
          <PageHistory />
        {:else if page === 'look'}
          <PageAppearance />
        {:else if page === 'conf'}
          <PageConfig />
        {:else}
          <PageAbout />
        {/if}
      </div>
    {/key}
  </main>
</div>

<ToastHost />

<style>
  :global(html), :global(body) { height: 100%; margin: 0; overflow: hidden; }
  .shell { display: flex; height: 100vh; background: var(--bg); color: var(--text); }
  nav {
    width: 228px;
    flex: none;
    box-sizing: border-box;
    padding: 16px 12px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    overflow-y: auto;
  }
  .brand { display: flex; align-items: center; gap: 12px; padding: 4px 12px 14px; }
  .logo {
    width: 38px; height: 38px; border-radius: 11px; flex: none;
    background: var(--accent); color: var(--on-accent, #fff);
    display: inline-flex; align-items: center; justify-content: center;
  }
  .logo svg { width: 21px; height: 21px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  .bt { display: flex; flex-direction: column; }
  .bt b { font-size: 15px; line-height: 20px; }
  .bt small { font-size: 12px; color: var(--text3); }
  .nav {
    position: relative;
    display: flex; align-items: center; gap: 12px;
    height: 36px; padding: 0 12px; width: 100%;
    border: 0; border-radius: 8px; background: transparent;
    font: inherit; font-size: 14px; color: var(--text); text-align: left; cursor: pointer;
    transition: background var(--d-hover) var(--ease-hover);
  }
  .nav:hover { background: var(--hover); }
  .nav.on { background: var(--hover); font-weight: 600; }
  .nav.on::before {
    content: ''; position: absolute; left: 0; top: 10px; width: 3px; height: 16px; border-radius: 2px;
    background: var(--accent); animation: bar 220ms cubic-bezier(.2, .9, .3, 1.2);
  }
  @keyframes bar { from { transform: scaleY(0); } }
  .nav svg { width: 18px; height: 18px; flex: none; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; color: var(--text2); }
  .nav.on svg { color: var(--accent); }
  main { flex: 1; min-width: 0; overflow-y: auto; box-sizing: border-box; padding: 20px 36px 40px 12px; }
  .page { max-width: 820px; display: flex; flex-direction: column; gap: 14px; }
</style>
