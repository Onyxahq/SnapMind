<script lang="ts">
  import { onMount } from 'svelte';
  import { appInfo, type AppInfo } from '$lib/ipc';

  let info = $state<AppInfo | null>(null);
  onMount(async () => {
    info = await appInfo();
  });
</script>

<main>
  <h1 class="t-title">SnapMind</h1>
  <p class="t-src">设置窗口（工程骨架）</p>
  {#if info}
    <p class="t-note">v{info.version} · {info.platform}</p>
  {/if}
</main>

<style>
  :global(html, body) { height: 100%; }
  main {
    height: 100%;
    display: grid;
    place-content: center;
    text-align: center;
  }
  h1, p { margin: 0; }
  p { margin-top: 4px; }
</style>
