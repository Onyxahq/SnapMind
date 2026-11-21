<script lang="ts">
  import { onMount } from 'svelte';
  import { appInfo, type AppInfo } from '$lib/ipc';

  let info = $state<AppInfo | null>(null);
  onMount(async () => {
    info = await appInfo();
  });
</script>

<main>
  <h1>SnapMind</h1>
  <p class="sub">设置窗口（工程骨架）</p>
  {#if info}
    <p class="meta">v{info.version} · {info.platform}</p>
  {/if}
</main>

<style>
  :global(html, body) {
    margin: 0;
    height: 100%;
    background: #e6ebf2;
    color: #0f172a;
    font-family: Inter, 'Noto Sans SC', 'Microsoft YaHei UI', sans-serif;
  }
  @media (prefers-color-scheme: dark) {
    :global(html, body) {
      background: #0e1219;
      color: #e8ecf2;
    }
  }
  main {
    height: 100%;
    display: grid;
    place-content: center;
    text-align: center;
  }
  h1 {
    margin: 0;
    font-size: 28px;
    font-weight: 650;
    letter-spacing: -0.02em;
  }
  .sub {
    margin: 8px 0 0;
    opacity: 0.7;
  }
  .meta {
    margin: 4px 0 0;
    font-size: 12px;
    opacity: 0.5;
  }
</style>
